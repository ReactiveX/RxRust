// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.

use net_stream::NetStream;

use mio::{
    EventLoop,
    EventLoopSender,
    EventLoopConfig,
    Handler,
    NonBlock,
    IoWriter,
    IoReader,
    IoAcceptor,
    Buf, MutBuf,
    Timeout,
    MioResult};

pub use mio::Token;
use mio::net::{SockAddr, Socket};
use mio::net::tcp::{TcpAcceptor, TcpSocket};
use mio::util::Slab;
use mio::event;

use iobuf::{Iobuf, RWIobuf, AROIobuf, Allocator, AppendBuf};

use std::old_io::net::addrinfo::get_host_addresses;
use std::result::Result;
use std::sync::Arc;
use std::sync::mpsc::{Receiver,SyncSender, sync_channel};

use std::time::Duration;

use collections::dlist::DList;

use reactive::Subscriber;
use protocol::Protocol;

/// The basic sendable buffer which also contains
/// its own addressing. When the buffer is received,
/// Token reflects the socket whence it came,
/// when it is sent, Token is the socket out of which
/// the buffer should be sent
#[derive(Show)]
pub struct StreamBuf (pub AROIobuf, pub Token);

#[derive(Show)]
pub struct ProtoMsg<T> (pub T, pub Token);

unsafe impl Send for StreamBuf {}

impl Clone for StreamBuf {
    fn clone(&self) -> StreamBuf {
        StreamBuf (self.0.clone(), self.1)
    }
}

struct ReadBuf (AppendBuf<'static>);

pub type TimerCB<'a> = FnMut(&mut Reactor)->bool + 'a;

pub type Reactor = EventLoop<Token, StreamBuf>;

pub type Sender = EventLoopSender<StreamBuf>;

impl Buf for StreamBuf {
    fn remaining(&self) -> usize {
        self.0.len() as usize
    }

    fn bytes<'a>(&'a self) -> &'a [u8] { unsafe {
        self.0.as_window_slice()
    } }

    fn advance(&mut self, cnt: usize) {
        self.0.advance(cnt as u32).unwrap();
    }
}

impl Buf for ReadBuf {
    fn remaining(&self) -> usize {
        self.0.len() as usize
    }

    fn bytes<'b>(&'b self) -> &'b [u8] {
        self.0.as_window_slice()
    }

    fn advance(&mut self, cnt: usize) {
        self.0.advance(cnt as u32).unwrap();
    }
}

impl MutBuf for ReadBuf {
    fn mut_bytes<'b>(&'b mut self) -> &'b mut [u8] {
        self.0.as_mut_window_slice()
    }
}


struct Connection<T>
where T : Protocol, <T as Protocol>::Output : Send
{
        sock: TcpSocket,
        outbuf: DList<StreamBuf>,
        interest: event::Interest,
        conn_tx: SyncSender<ProtoMsg<<T as Protocol>::Output>>,
        marker: u32,
        proto: T,
        buf: ReadBuf
}

impl<T> Connection<T>
where T : Protocol, <T as Protocol>::Output : Send
{
    pub fn new(s: TcpSocket, tx: SyncSender<ProtoMsg<<T as Protocol>::Output>>, rbuf: ReadBuf) -> Connection<T> {
        Connection {
            sock: s,
            outbuf: DList::new(),
            interest: event::HUP,
            conn_tx: tx,
            marker: 0,
            proto: <T as Protocol>::new(),
            buf:  rbuf
        }
    }

    fn drain_write_queue_to_socket(&mut self) -> usize {
        let mut writable = true;
        while writable && self.outbuf.len() > 0 {
            let (result, sz) = {
                let buf = self.outbuf.front_mut().unwrap(); //shouldn't panic because of len() check
                let sz = buf.0.len();
                (self.sock.write(buf), sz as usize)
            };
            match result {
                Ok(NonBlock::Ready(n)) =>
                {
                    debug!("Wrote {:?} out of {:?} bytes to socket", n, sz);
                    if n == sz {
                        self.outbuf.pop_front(); // we have written the contents of this buffer so lets get rid of it
                    }
                },
                Ok(NonBlock::WouldBlock) => { // this is also very unlikely, we got a writable message, but failed
                    // to write anything at all.
                    debug!("Got Writable event for socket, but failed to write any bytes");
                    writable = false;
                },
                Err(e) => { error!("error writing to socket: {:?}", e); writable = false }
            }
        }
        self.outbuf.len()
    }

    fn read(&mut self) -> MioResult<NonBlock<usize>> {
        self.sock.read(&mut self.buf)
    }
}


/// Configuration for the Net Engine
/// queue_size: All queues, both inbound and outbound
/// read_buf_sz: The size of the read buffer allocatod
pub struct NetEngineConfig {
    queue_size: usize,
    read_buf_sz: usize,
    min_read_buf_sz: usize,
    max_connections: usize,
    poll_timeout_ms: usize,
    allocator: Option<Arc<Box<Allocator>>>
}

pub struct NetEngine<'a, T>
where T : Protocol, <T as Protocol>::Output : Send
{
    inner: EngineInner<'a, T>,
    event_loop: Reactor
}


impl<'a, T> NetEngine<'a, T>
where T : Protocol, <T as Protocol>::Output : Send
{

    /// Construct a new NetEngine with (hopefully) intelligent defaults
    ///
    pub fn new() -> NetEngine<'a, T> {
        let config = NetEngineConfig {
            queue_size: 524288,
            read_buf_sz: 1536,
            min_read_buf_sz: 64,
            allocator: None,
            max_connections: 10240,
            poll_timeout_ms: 100
        };

        NetEngine::configured(config)
    }

    /// Construct a new engine with defaults specified by the user
    pub fn configured(cfg: NetEngineConfig) -> NetEngine<'a, T> {
        NetEngine { event_loop: EventLoop::configured(NetEngine::<'a, T>::event_loop_config(cfg.queue_size, cfg.poll_timeout_ms)).unwrap(),
                    inner: EngineInner::new(cfg)
        }
    }

    fn event_loop_config(queue_sz : usize, timeout: usize) -> EventLoopConfig {
        EventLoopConfig {
            io_poll_timeout_ms: timeout,
            notify_capacity: queue_sz,
            messages_per_tick: 512,
            timer_tick_ms: 100,
            timer_wheel_size: 1_024,
            timer_capacity: 65_536,
        }
    }

    /// connect to the supplied hostname and port
    /// any data that arrives on the connection will be put into a Buf
    /// and sent down the supplied Sender channel along with the Token of the connection
    pub fn connect<'b>(&mut self,
                   hostname: &str,
                   port: usize) -> Result<NetStream<'b, <T as Protocol>::Output>, String> {
        self.inner.connect(hostname, port, &mut self.event_loop)
    }

    /// listen on the supplied ip address and port
    /// any new connections will be accepted and polled for read events
    /// all datagrams that arrive will be put into StreamBufs with their
    /// corresponding token, and added to the default outbound data queue
    /// this can be called multiple times for different ips/ports
    pub fn listen<'b>(&mut self,
                  addr: &'b str,
                  port: usize) -> Result<Receiver<ProtoMsg<<T as Protocol>::Output>>, String> {
        self.inner.listen(addr, port, &mut self.event_loop)
    }

    /// fetch the event_loop channel for notifying the event_loop of new outbound data
    pub fn channel(&self) -> EventLoopSender<StreamBuf> {
        self.event_loop.channel()
    }

    /// Set a timeout to be executed by the event loop after duration
    /// Minimum expected resolution is the tick duration of the event loop
    /// poller, but it could be shorted depending on how many events are
    /// occurring
    pub fn timeout(&mut self, timeout: Duration, callback: Box<TimerCB<'a>>) {
        let tok = self.inner.timeouts.insert((callback, None)).map_err(|_|()).unwrap();
        let handle = self.event_loop.timeout(tok, timeout).unwrap();
        self.inner.timeouts.get_mut(tok).unwrap().1 = Some(handle);
    }

    /// process all incoming and outgoing events in a loop
    pub fn run(mut self) {
        self.event_loop.run(self.inner).map_err(|_| ()).unwrap();
    }

    /// process all incoming and outgoing events in a loop
    pub fn run_once(mut self) {
        self.event_loop.run_once(self.inner).map_err(|_| ()).unwrap();
    }

    /// calculates the 11th digit of pi
    pub fn shutdown(mut self) {
        self.event_loop.shutdown();
    }
}


struct EngineInner<'a, T>
where T : Protocol, <T as Protocol>::Output : Send
{
    listeners: Slab<(TcpAcceptor, SyncSender<ProtoMsg<<T as Protocol>::Output>>)>,
    timeouts: Slab<(Box<TimerCB<'a>>, Option<Timeout>)>,
    conns: Slab<Connection<T>>,
    config: NetEngineConfig,
}

impl<'a, T> EngineInner<'a, T>
where T : Protocol, <T as Protocol>::Output : Send
{

    pub fn new(cfg: NetEngineConfig) -> EngineInner<'a, T> {

        EngineInner {
            listeners: Slab::new_starting_at(Token(0), 128),
            timeouts: Slab::new_starting_at(Token(129), 255),
            conns: Slab::new_starting_at(Token(256), cfg.max_connections + 256),
            config: cfg
        }
    }

    pub fn connect<'b>(&mut self,
                   hostname: &str,
                   port: usize,
                   event_loop: &mut Reactor) -> Result<NetStream<'b, <T as Protocol>::Output>, String>
    {
        let ip = get_host_addresses(hostname).unwrap()[0]; //TODO manage receiving multiple IPs per hostname, random sample or something
        match TcpSocket::v4() {
            Ok(s) => {
                //s.set_tcp_nodelay(true); TODO: re-add to mio
                let (tx, rx) = sync_channel(self.config.queue_size);
                let buf = new_buf(self.config.read_buf_sz, self.config.allocator.clone());
                match self.conns.insert(Connection::new(s, tx, buf)) {
                    Ok(tok) => match event_loop.register_opt(&self.conns.get(tok).unwrap().sock, tok, event::READABLE, event::PollOpt::edge()) {
                        Ok(..) => match self.conns.get(tok).unwrap().sock.connect(&SockAddr::InetAddr(ip, port as u16)) {
                            Ok(..) => {
                                debug!("Connected to server for token {:?}", tok);
                                Ok(NetStream::new(tok, rx, event_loop.channel().clone()))
                            }
                            Err(e) => Err(format!("Failed to connect to {:?}:{:?}, error: {:?}", hostname, port, e))
                        },
                        Err(e)      => Err(format!("Failed to register with the event loop, error: {:?}", e))
                    },
                    _ => Err(format!("Failed to insert into connection slab"))
                }
            },
            Err(e) => Err(format!("Failed to create new socket, error:{:?}", e))
        }
    }

    pub fn listen<'b>(&mut self,
                  addr: &'b str,
                  port: usize,
                  event_loop: &mut Reactor) -> Result<Receiver< ProtoMsg< <T as Protocol>::Output >>, String>
    {
        let ip = get_host_addresses(addr).unwrap()[0];
        match TcpSocket::v4() {
            Ok(s) => {
                //s.set_tcp_nodelay(true); TODO: re-add to mio
                match s.bind(&SockAddr::InetAddr(ip, port as u16)) {
                Ok(l) => match l.listen(255) {
                    Ok(a) => {
                        let (tx, rx) = sync_channel(self.config.queue_size);
                        match self.listeners.insert((a, tx)) {
                            Ok(token) => {
                                event_loop.register_opt(&self.listeners.get_mut(token).unwrap().0,
                                                        token,
                                                        event::READABLE,
                                                        event::PollOpt::edge()).
                                                            map_err(|e| format!("event registration failed: {:?}", e)).
                                                            map(move |_| rx)
                            },
                            Err(_) => Err(format!("failed to insert into listener slab"))
                        }
                    },
                    Err(e) => {Err(format!("Failed to listen to socket {:?}:{:?}, error:{:?}", addr, port, e)) }
                },
                Err(e) => Err(format!("Failed to bind to {:?}:{:?}, error:{:?}", addr, port, e))
            }},
            Err(e) => Err(format!("Failed to create TCP socket, error:{:?}", e))
        }
    }


}

impl<'a, T> Handler<Token, StreamBuf> for EngineInner<'a, T>
where T : Protocol, <T as Protocol>::Output : Send
{

    fn readable(&mut self, event_loop: &mut Reactor, token: Token, hint: event::ReadHint) {
        debug!("mio_processor::readable top, token: {:?}", token);
        let mut close = false;
        if self.listeners.contains(token) {
            let calloc = &self.config.allocator;
            let buf_sz = self.config.read_buf_sz;
            let (ref mut list, ref tx) = *self.listeners.get_mut(token).unwrap();
                match list.accept() {
                    Ok(NonBlock::Ready(sock)) => {
                        let buf = new_buf(buf_sz, calloc.clone());
                        match self.conns.insert(Connection::new(sock, tx.clone(), buf)) {
                            Ok(tok) =>  {
                                event_loop.register_opt(&self.conns.get(tok).unwrap().sock,
                                                        tok, event::READABLE | event::HUP,
                                                        event::PollOpt::edge()).unwrap();
                                          debug!("readable accepted socket for token {:?}", tok); }
                            Err(..)  => error!("Failed to insert into Slab")
                        }; },
                    e => { error!("Failed to accept socket: {:?}", e);}
                }
            event_loop.reregister(list, token, event::READABLE, event::PollOpt::edge()).unwrap();
            return;

        } else {

            match self.conns.get_mut(token) {
                None    => error!("Got a readable event for token {:?},
                                   but it is not present in MioHandler connections", token),
                Some(c) => {
                    match c.read() {
                        Ok(NonBlock::Ready(n)) => {
                            debug!("read {:?} bytes", n);
                            let mut abuf = c.buf.0.atomic_slice_pos_from_begin(c.marker, n as i64).unwrap();
                            loop {
                                match c.proto.append(&abuf) {
                                    None => {break},
                                    Some((item, remaining, consumed)) => {
                                        c.conn_tx.send( ProtoMsg(item, token));
                                        abuf = remaining;
                                        c.marker += consumed;
                                    }
                                }
                            }
                            if c.buf.0.len() < self.config.min_read_buf_sz as u32 {
                                let mut newbuf = new_buf(self.config.read_buf_sz, self.config.allocator.clone());
                                if abuf.len() > 0 {
                                    // we didn't eat all of the bytes we just read
                                    // so we must move them to the new buffer
                                    unsafe { newbuf.0.fill(abuf.as_window_slice()) };
                                }
                                c.buf = newbuf;
                                c.marker = 0;
                            }
                        }
                        Ok(NonBlock::WouldBlock) => {
                            debug!("Got Readable event for socket, but failed to write any bytes");
                        },
                        Err(e) => error!("error reading from socket: {:?}", e)
                    };

                    if hint.contains(event::HUPHINT) {
                        close = true;
                    }
                    else {
                        c.interest.insert(event::READABLE);
                        event_loop.reregister(&c.sock, token, c.interest, event::PollOpt::edge()).unwrap();
                    }
                }
            }

            if close {
                self.conns.remove(token);
            }
        }
    }

    fn writable(&mut self, event_loop: &mut Reactor, token: Token) {
        debug!("mio_processor::writable, token: {:?}", token);
        if let Some(c) = self.conns.get_mut(token) {
            if c.drain_write_queue_to_socket() > 0 {
                    c.interest.insert(event::WRITABLE);
                    event_loop.reregister(&c.sock, token, c.interest, event::PollOpt::edge()).unwrap();
            }
        }
    }


    fn notify(&mut self, event_loop: &mut Reactor, msg: StreamBuf) {
        let tok = msg.1;
        match self.conns.get_mut(tok) {
            Some(c) => {
                c.outbuf.push_back(msg);
                if c.drain_write_queue_to_socket() > 0 {
                    c.interest.insert(event::WRITABLE);
                    event_loop.reregister(&c.sock, tok, c.interest, event::PollOpt::edge()).unwrap();
                }
            },
            None => {}
        }
    }

    fn timeout(&mut self, event_loop: &mut Reactor, tok: Token) {
        let (ref mut cb, ref handle) = *self.timeouts.get_mut(tok).unwrap();
        if !(*cb).call_mut((event_loop,)) {
            event_loop.clear_timeout(handle.unwrap());
        }
    }
}

fn new_buf(sz: usize, calloc: Option<Arc<Box<Allocator>>>) -> ReadBuf {
    if let Some(alloc) = calloc {
        ReadBuf(AppendBuf::new_with_allocator(sz, alloc))
    } else {
        ReadBuf(AppendBuf::new(sz))
    }
}
