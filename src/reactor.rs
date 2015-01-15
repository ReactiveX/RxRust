// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.

use net_stream::NetStream;

use mio::{
    EventLoop,
    EventLoopSender,
    Handler,
    NonBlock,
    IoWriter,
    IoReader,
    IoAcceptor,
    Timeout};

pub use mio::Token;
use mio::net::{SockAddr, Socket};
use mio::net::tcp::{TcpAcceptor, TcpSocket};
use mio::util::Slab;
use mio::event;

use iobuf::{Iobuf, RWIobuf, AROIobuf, Allocator};

use std::io::net::addrinfo::get_host_addresses;
use std::result::Result;
use std::sync::Arc;
use std::sync::mpsc::{Receiver,SyncSender, sync_channel};

use std::time::Duration;

use collections::dlist::DList;

use reactive::Subscriber;
/// The basic sendable buffer which also contains
/// its own addressing. When the buffer is received,
/// Token reflects the socket whence it came,
/// when it is sent, Token is the socket out of which
/// the buffer should be sent
#[derive(Show)]
pub struct StreamBuf (pub AROIobuf, pub Token);

unsafe impl Send for StreamBuf {}

impl Clone for StreamBuf {
    fn clone(&self) -> StreamBuf {
        StreamBuf (self.0.clone(), self.1)
    }
}

pub type TimerCB<'a> = FnMut(&mut Reactor)->bool + 'a;

pub type Reactor = EventLoop<Token, StreamBuf>;

pub type Sender = EventLoopSender<StreamBuf>;

struct Connection {
        sock: TcpSocket,
        outbuf: DList<AROIobuf>,
        interest: event::Interest,
        conn_tx: SyncSender<StreamBuf>
}

impl Connection {
    pub fn new(s: TcpSocket, tx: SyncSender<StreamBuf>) -> Connection {
        Connection {
            sock: s,
            outbuf: DList::new(),
            interest: event::HUP,
            conn_tx: tx
        }
    }
}

pub struct NetEngine<'a> {
    inner: EngineInner<'a>,
    event_loop: Reactor
}

impl<'a> NetEngine<'a> {
    /// * buf_sz: the size of the buffer that will be used to read from and send to sockets
    ///         this should be set to the size of your MTU, likely 1500 bytes
    /// * max_conns: Size for a pre-allocated array of containers for Connection objects
    /// * max_queue_sz: Size of the default sync channel through which buffers are sent
    pub fn new(buf_sz: usize, max_conns: usize, max_queue_sz: usize) -> NetEngine<'a> {
        NetEngine { inner: EngineInner::new(buf_sz, max_conns, max_queue_sz),
                    event_loop: EventLoop::new().unwrap() }
    }

    /// * buf_sz: the size of the buffer that will be used to read from and send to sockets
    ///         this should be set to the size of your MTU, likely 1500 bytes
    /// * max_conns: Size for a pre-allocated array of containers for Connection objects
    /// * max_queue_sz: Size of the default sync channel through which buffers are sent
    /// * a: Specialized allocator for the buffers to be read
    pub fn new_with_alloc(buf_sz: usize,
               max_conns: usize,
               max_queue_sz: usize,
               a: Arc<Box<Allocator>>) -> NetEngine<'a> {
        NetEngine { inner: EngineInner::new_with_alloc(buf_sz, max_conns, max_queue_sz, a),
                    event_loop: EventLoop::new().unwrap() }
     }

    /// connect to the supplied hostname and port
    /// any data that arrives on the connection will be put into a Buf
    /// and sent down the supplied Sender channel along with the Token of the connection
    pub fn connect<'b>(&mut self,
                   hostname: &str,
                   port: usize) -> Result<NetStream<'b>, String> {
        self.inner.connect(hostname, port, &mut self.event_loop)
    }

    /// listen on the supplied ip address and port
    /// any new connections will be accepted and polled for read events
    /// all datagrams that arrive will be put into StreamBufs with their
    /// corresponding token, and added to the default outbound data queue
    /// this can be called multiple times for different ips/ports
    pub fn listen<'b>(&mut self,
                  addr: &'b str,
                  port: usize) -> Result<Receiver<StreamBuf>, String> {
        self.inner.listen(addr, port, &mut self.event_loop)
    }

    /// fetch the event_loop channel for notifying the event_loop of new outbound data
    pub fn channel(&self) -> EventLoopSender<StreamBuf> {
        self.event_loop.channel()
    }

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

struct EngineInner<'a> {
    listeners: Slab<(TcpAcceptor, SyncSender<StreamBuf>)>,
    timeouts: Slab<(Box<TimerCB<'a>>, Option<Timeout>)>,
    conns: Slab<Connection>,
    alloc: Option<Arc<Box<Allocator>>>,
    buf_sz: usize,
    queue_sz: usize
}

impl<'a> EngineInner<'a> {

    pub fn new(buf_sz: usize, max_conns: usize, max_queue_sz: usize) -> EngineInner<'a> {
        EngineInner {
            listeners: Slab::new_starting_at(Token(0), 128),
            timeouts: Slab::new_starting_at(Token(129), 255),
            conns: Slab::new_starting_at(Token(256), max_conns + 256),
            buf_sz: buf_sz,
            queue_sz: max_queue_sz,
            alloc: None
        }
    }

    pub fn new_with_alloc(buf_sz: usize,
               max_conns: usize,
               max_queue_sz: usize,
               a: Arc<Box<Allocator>>) -> EngineInner<'a> {
        let ns = EngineInner::new(buf_sz, max_conns, max_queue_sz);
        EngineInner { alloc: Some(a), ..ns }
    }

    pub fn connect<'b>(&mut self,
                   hostname: &str,
                   port: usize,
                   event_loop: &mut Reactor) -> Result<NetStream<'b>, String>
    {
        let ip = get_host_addresses(hostname).unwrap()[0]; //TODO manage receiving multiple IPs per hostname, random sample or something
        match TcpSocket::v4() {
            Ok(s) => {
                //s.set_tcp_nodelay(true); TODO: re-add to mio
                let (tx, rx) = sync_channel(self.queue_sz);
                match self.conns.insert(Connection::new(s, tx)) {
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
                  event_loop: &mut Reactor) -> Result<Receiver<StreamBuf>, String>
    {
        let ip = get_host_addresses(addr).unwrap()[0];
        match TcpSocket::v4() {
            Ok(s) => {
                //s.set_tcp_nodelay(true); TODO: re-add to mio
                match s.bind(&SockAddr::InetAddr(ip, port as u16)) {
                Ok(l) => match l.listen(255) {
                    Ok(a) => {
                        let (tx, rx) = sync_channel(self.queue_sz);
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

impl<'a> Handler<Token, StreamBuf> for EngineInner<'a> {

    fn readable(&mut self, event_loop: &mut Reactor, token: Token, hint: event::ReadHint) {
        debug!("mio_processor::readable top, token: {:?}", token);
        let mut close = false;
        if self.listeners.contains(token) {
            let (ref mut list, ref tx) = *self.listeners.get_mut(token).unwrap();
                match list.accept() {
                    Ok(NonBlock::Ready(sock)) => {
                        match self.conns.insert(Connection::new(sock, tx.clone())) {
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

                    let mut b =
                        if let Some(alloc) = self.alloc.as_ref() {
                            RWIobuf::new_with_allocator(self.buf_sz, alloc.clone())
                        } else {
                            RWIobuf::new(self.buf_sz)
                        };

                    let result = unsafe { c.sock.read_slice(b.as_mut_window_slice()) };
                    match result {
                        Ok(NonBlock::Ready(n)) => {debug!("read {:?} bytes", n);
                                  b.advance(n as u32).unwrap();
                                  b.flip_lo();
                                  let abuf = b.atomic_read_only().unwrap();
                                  c.conn_tx.send( StreamBuf (abuf, token) ).unwrap(); },
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
        match self.conns.get_mut(token) {
            None    => error!("{:?} not present in connections", token),
            Some(c) => { 
                let mut writable = true;
                let mut index : usize = 0;
                while writable && c.outbuf.len() > 0 {
                    let (result, sz) =
                        unsafe {
                            let buf = c.outbuf.front_mut().unwrap();
                            let sz = buf.len() - index as u32;
                            let b : &[u8] = &buf.as_window_slice()[index..];
                            (c.sock.write_slice(b), sz)
                        };
                    match result {
                        Ok(NonBlock::Ready(n)) =>
                        {
                            debug!("Wrote {:?} out of {:?} bytes to socket", n, sz);
                            if n == sz as usize {
                                c.outbuf.pop_front(); // we have written the contents of this buffer so lets get rid of it
                                index = 0;
                            }
                            else { // this is unlikely to happen, we didn't write all of the
                                // buffer to the socket, so lets try again
                                index += n;
                            }
                        },
                        Ok(NonBlock::WouldBlock) => { // this is also very unlikely, we got a writable message, but failed
                            // to write anything at all.
                            debug!("Got Writable event for socket, but failed to write any bytes");
                            writable = false;
                        },
                        Err(e)              => { error!("error writing to socket: {:?}", e); writable = false }
                    }
                }
                if c.outbuf.len() > 0 {
                    c.interest.insert(event::WRITABLE);
                    event_loop.reregister(&c.sock, token, c.interest, event::PollOpt::edge()).unwrap();
                }
            },
        }
    }

    fn notify(&mut self, event_loop: &mut Reactor, msg: StreamBuf) {
        let StreamBuf (buf, tok) = msg;
        match self.conns.get_mut(tok) {
            Some(c) => { c.outbuf.push_back(buf);
                         c.interest.insert(event::WRITABLE);
                         event_loop.reregister(&c.sock, tok, c.interest, event::PollOpt::edge()).unwrap();},
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

