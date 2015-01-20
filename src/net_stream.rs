// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.
//! A handler for both incoming and outgoing connections,
//! all incoming connections' data is sent through a standard sync_channel
//! which is returned from EngineInner::new
//! all outgoing connections produce their own sync channel through which
//! their incoming data is received.  Outgoing messages are all sent
//! back through the sender provided by EngineInner::channel or via the
//! StreamConneciton send_all function for Traversals

use reactor::{Reactor, StreamBuf, Sender};
use mio::Token;
use publisherimpl::Coupler;
use reactive::{Publisher, Subscriber};
use iobuf::{AROIobuf};

use std::sync::mpsc::{Receiver,SyncSender};
use std::sync::Arc;
use std::cell::RefCell;

type Superbox<T> = Arc<RefCell<Box<T>>>;

/// A Traversal style representation of a socket
#[derive(Clone)]
pub struct NetStream<'a> {
    pub dtx: Sender,
    pub drx: Arc<Receiver<StreamBuf>>,
    pub tok: Token,
}


impl<'a> NetStream<'a>
{
    pub fn new(tok: Token,
               drx: Receiver<StreamBuf>,
               dtx: Sender) -> NetStream<'a> {
        NetStream { tok: tok, drx: Arc::new(drx), dtx: dtx.clone() }
    }
}

pub struct NetStreamer<'a>
{
    stream: NetStream<'a>,
    subscriber: Option<Box<Subscriber<Input=StreamBuf> + 'a >>
    //subscriber: Option<Box<Subscriber<Input=<NetStreamer<'a> as Publisher<'a>>::Output> + 'a >>
}

impl<'a> Subscriber for NetStreamer<'a>
{
    type Input = StreamBuf;
    fn on_next(&mut self, StreamBuf (buf, _, ctl) : StreamBuf) -> bool {

        //TODO better handle queue failure, maybe put the returned buf
        //isizeo a recovery queue
        match self.stream.dtx.send(StreamBuf(buf, self.stream.tok, ctl)) {
            Ok(()) => true,
            Err(_) => false
        }
    }
}

impl<'a> Publisher<'a> for NetStreamer<'a>
{
    type Output = StreamBuf;

    //fn subscribe(&mut self, s: Box<Subscriber<Input=<Self as Publisher<'a>>::Output > + 'a>) {
    fn subscribe(&mut self, s: Box<Subscriber<Input=StreamBuf> + 'a>) {
        //let t: Box<Subscriber<Input=<Self as Publisher<'a>>::Output> + 'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }

    fn next (&mut self) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => match self.stream.drx.recv() {
                Ok(d) => s.on_next(d),
                Err(..) => { s.on_complete(false); false }
            },
            None => { error!("My subscriber went away"); false }
        }
    }
}

#[cfg(test)]
mod test {
use reactor::Reactor;
use reactor::{StreamBuf, NetEngine};
use std::thread::Thread;
use std::vec::Vec;
use std::mem;
use std::num::Int;
use std::raw;
use std::io::timer::sleep;
use mio::Token;
use iobuf::{Iobuf, RWIobuf};
use std::time::Duration;
use publisher::{Repeat, Coupler};
use processor::{Map, Take, Trace};
use subscriber::{Decoupler, Collect};
use reactive::{Publisher, Subscriber};

    unsafe fn to_bytes<T>(t: &T) -> &[u8] {
        mem::transmute(raw::Slice::<u8> {
            data: t as *const T as *const u8,
            len:  mem::size_of::<T>(),
        })
    }

    fn isize_to_strbuf<T : Int>(t: &T) -> StreamBuf { unsafe {
        StreamBuf (RWIobuf::from_slice_copy(to_bytes(t)).atomic_read_only().unwrap(), Token(0))
    }}

    fn strbuf_to_isize<T : Int>(buf: StreamBuf) -> T { unsafe {
        *(mem::transmute::<*mut u8, *const T>(buf.0.ptr()))
    }}

    #[test]
    fn oneway_test() {

        let mut ne = NetEngine::new(8, 100, 100);
        let srv_rx = ne.listen("127.0.0.1", 10000).unwrap();
        let cl = { ne.connect("127.0.0.1", 10000).unwrap().clone() };

        ne.timeout(Duration::milliseconds(500), Box::new(|&: el : &mut Reactor| { el.shutdown(); true}));

        let tok = cl.tok.clone();
        let dtx = cl.dtx.clone();

        Thread::spawn(move|| {
            let mut rep = Box::new(Repeat::new(5u64));
            let mut map1 = Box::new(Map::new(|x| isize_to_strbuf(&x)));
            let mut map2 = Box::new(Map::new(move |StreamBuf (buf, _)| StreamBuf (buf, tok)));
            let mut trc  = Box::new(Trace::new());
            let mut sen = Box::new(Decoupler::new(dtx));

            trc.subscribe(sen);
            map2.subscribe(trc);
            map1.subscribe(map2);
            rep.subscribe(map1);

            for i in range(0, 5) {
                rep.next();
            }

            let mut v = Box::new(Vec::<u64>::new());
            {
                let mut recv = Box::new(Coupler::new(srv_rx));
                let mut take = Box::new(Take::new(5));
                let mut map3 = Box::new(Map::new(|x| strbuf_to_isize(x)));
                let mut coll = Box::new(Collect::new(&mut v));

                map3.subscribe(coll);
                take.subscribe(map3);
                recv.subscribe(take);
                recv.run();
            }
            assert_eq!(*v, vec![5,5,5,5,5]);

        });

        ne.run();
    }
    /*
    #[test]
    fn roundtrip_test() {

        let mut ne = NetEngine::new(1500, 100, 100);
        let srv_rx = ne.listen("127.0.0.1", 10001).unwrap();
        let cl = ne.connect("127.0.0.1", 10001).unwrap();

        ne.timeout(Duration::milliseconds(100), Box::new(|el| { el.shutdown(); true});

        Thread::spawn(move|| {
            T::range::<isize>(0, 5).map(|x| isize_to_strbuf(&x)).send(cl.dtx);
            assert_eq!(recv(srv_rx).map(|x| strbuf_to_isize(x)).collect::<Vec<isize>>(), vec![0, 1, 2, 3, 4]);


            assert_eq!(recv(cl.drx).map(|x| strbuf_to_isize(x)).collect::<Vec<isize>>(), vec![0, 1, 2, 3, 4]);
        }).detach();

        ne.run();
    }
    */
}

