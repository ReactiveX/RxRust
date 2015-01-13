use collections::dlist::DList;

use mio::{EventLoop, Token, Handler, NonBlock, IoWriter, IoReader, IoAcceptor, MioErrorKind, MioError};
use mio::net::{SockAddr};
use mio::net::tcp::{TcpAcceptor, TcpSocket};
use mio::util::Slab;
use mio::event;

use iobuf::{Iobuf, RWIobuf, AROIobuf, Allocator};
use std::io::net::addrinfo::get_host_addresses;
use std::result::Result;
use std::sync::Arc;
use std::sync::mpsc::{Sender, Receiver};

static LISTENER : Token = Token(-3);


pub struct Connection {
        sock: TcpSocket,
        outbuf: DList<AROIobuf>,
        isizeerest: event::Interest,
        data_tx: Sender<(AROIobuf, Token)>
}

impl Connection {
    pub fn new(s: TcpSocket, tx: Sender<(AROIobuf, Token)>) -> Connection {
        Connection {
            sock: s,
            outbuf: DList::new(),
            isizeerest: event::HUP,
            data_tx: tx
        }
    }
}

pub struct NetProcessor {
    listener: Option<TcpAcceptor>,
    conns: Slab<Connection>,
    data_tx: Sender<(AROIobuf, Token)>,
    alloc: Option<Arc<Box<Allocator>>>,
    buf_sz: usize
}

impl NetProcessor {
    pub fn new(buf_sz: usize, tx: Sender<(AROIobuf, Token)>) -> NetProcessor {
        NetProcessor {
            listener: None,
            conns: Slab::new_starting_at(Token(2), 128),
            data_tx: tx,
            buf_sz: buf_sz,
            alloc: None
        }
    }

    pub fn new_with_alloc(buf_sz: usize,
               tx: Sender<(AROIobuf, Token)>,
               a: Arc<Box<Allocator>>) -> NetProcessor {
        NetProcessor {
            listener: None,
            conns: Slab::new_starting_at(Token(2), 128),
            data_tx: tx,
            buf_sz: buf_sz,
            alloc: Some(a)
        }
    }

    // connect to the supplied hostname and port
    // any data that arrives on the connection will be put isizeo a Buf
    // and sent down the supplied Sender channel along with the Token of the connection
    pub fn connect(&mut self,
                   hostname: &'static str,
                   port: usize,
                   tx: Sender<(AROIobuf, Token)>,
                   event_loop: &mut EventLoop<usize, (AROIobuf, Token)>) -> Result<Token, String>
    {
        let ip = get_host_addresses(hostname).unwrap()[0]; //TODO manage receiving multiple IPs per hostname, random sample or something
        match TcpSocket::v4() {
            Ok(s) => {
                match self.conns.insert(Connection::new(s, tx)) {
                    Ok(tok) => match event_loop.register_opt(&self.conns.get(tok).unwrap().sock, tok, event::READABLE, event::PollOpt::edge()) {
                        Ok(..) => match self.conns.get(tok).unwrap().sock.connect(&SockAddr::InetAddr(ip, port as u16)) {
                            Ok(..) => {debug!("Connected to server for token {:?}", tok); Ok(tok) }
                            Err(e) => Err(format!("Failed to connect to {:?}:{:?}, error: {:?}", hostname, port, e))
                        },
                        Err(e)      => Err(format!("Failed to register with the event loop, error: {:?}", e))
                    },
                    _ => Err(format!("Failed to insert isizeo connection slab"))
                }
            },
            Err(e) => Err(format!("Failed to create new socket, error:{:?}", e))
        }
    }

    pub fn listen(&mut self,
                  addr: &'static str,
                  port: usize,
                  event_loop: &mut EventLoop<usize, (AROIobuf, Token)>) -> Result<(), String>
    {
        let ip = get_host_addresses(addr).unwrap()[0];
        match TcpSocket::v4() {
            Ok(s) => {
                event_loop.register_opt(&s, LISTENER, event::READABLE, event::PollOpt::edge()).unwrap();
                match s.bind(&SockAddr::InetAddr(ip, port as u16)) {
                Ok(l) => match l.listen(255) {
                    Ok(a) => {self.listener = Some(a);
                              debug!("successfully listening on {:?}:{:?}", ip, port);
                              Ok(())}
                    Err(e) => {Err(format!("Failed to listen to socket {:?}:{:?}, error:{:?}", addr, port, e)) }
                },
                Err(e) => Err(format!("Failed to bind to {:?}:{:?}, error:{:?}", addr, port, e))
            }},
            Err(e) => Err(format!("Failed to create TCP socket, error:{:?}", e))
        }
    }
}

impl Handler<usize, (AROIobuf, Token)> for NetProcessor {

    fn readable(&mut self, event_loop: &mut EventLoop<usize, (AROIobuf, Token)>, token: Token, hisize: event::ReadHisize) {
        debug!("mio_processor::readable top, token: {:?}", token);
        let mut close = false;
        if token == LISTENER {
            match self.listener.as_mut().expect("Got a read event for listener but there is no listener!").accept() {
                Ok(NonBlock::Ready(sock)) => match self.conns.insert(Connection::new(sock, self.data_tx.clone())) {
                    Ok(tok) =>  { event_loop.register_opt(&self.conns.get(tok).unwrap().sock, tok, event::READABLE | event::HUP, event::PollOpt::edge()).unwrap();
                                  debug!("readable accepted socket for token {:?}", tok); }
                    Err(..)  => error!("Failed to insert isizeo Slab")
                },
                _ => error!("Failed to accept socket")
            }
            event_loop.reregister(self.listener.as_mut().expect("Meh"), LISTENER, event::READABLE, event::PollOpt::edge()).unwrap();
            return;
        }

        match self.conns.get_mut(token) {
            None    => error!("Got a readable event for token {:?}, but it is not present in MioHandler connections", token),
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
                              c.data_tx.send( (abuf, token) ).unwrap(); },
                    Ok(NonBlock::WouldBlock) => {
                                      // this is also very unlikely, we got a readable event, but failed
                                      // to read anything at all.
                                      debug!("Got Readable event for socket, but failed to read any bytes");
                    },
                    Err(MioError{kind: MioErrorKind::Eof, ..}) => debug!("Counter party disconnected"),
                    Err(e) => error!("Unknown error in receiving: {:?}", e)
                };

                if hisize.contains(event::HUPHINT) {
                    close = true;
                }
                else {
                    c.isizeerest.insert(event::READABLE);
                    event_loop.reregister(&c.sock, token, c.isizeerest, event::PollOpt::edge()).unwrap();
                }
            }
        }

        if close {
            self.conns.remove(token);
        }
    }

    fn writable(&mut self, event_loop: &mut EventLoop<usize, (AROIobuf, Token)>, token: Token) {
        debug!("mio_processor::writable, token: {:?}", token);
        match self.conns.get_mut(token) {
            None    => error!("Got message to write to token {:?}, but it is not present in MioHandler connections", token),
            Some(c) => { let mut writable = true;
                         let mut index : usize = 0;
                         while writable && c.outbuf.len() > 0 {
                             let (result, sz) =
                             unsafe {
                                 let buf = c.outbuf.front_mut().unwrap();
                                 let sz = buf.len() - index as u32;
                                 (c.sock.write_slice(buf.as_window_slice()[index..]), sz)
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
                                Err(e)              => error!("error writing to socket: {:?}", e)
                             }
                         }
                         if c.outbuf.len() > 0 {
                             c.isizeerest.insert(event::WRITABLE);
                             event_loop.reregister(&c.sock, token, c.isizeerest, event::PollOpt::edge()).unwrap();
                         }
            },
        }

    }

    fn notify(&mut self, event_loop: &mut EventLoop<usize, (AROIobuf, Token)>, msg: (AROIobuf, Token)) {
        debug!("mio_processor::notify top");
        let (buf, tok) = msg;
        match self.conns.get_mut(tok) {
            Some(c) => { c.outbuf.push_back(buf);
                         c.isizeerest.insert(event::WRITABLE);
                         event_loop.reregister(&c.sock, tok, c.isizeerest, event::PollOpt::edge()).unwrap();},
            None => {:?}
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<usize, (AROIobuf, Token)>, _: usize) {
    }
}


