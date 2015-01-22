#![feature(unboxed_closures)]

extern crate mio;
extern crate iobuf;
extern crate rx;
extern crate alloc;
extern crate time;

#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

use iobuf::{RWIobuf, AROIobuf, Iobuf, Allocator};
use mio::{EventLoop, EventLoopSender, Token};

use alloc::heap as a;
use std::sync::Arc;

use rx::reactive::{Publisher, Subscriber};
use rx::publisher::{IterPublisher, Coupler};
use rx::subscriber::{StdoutSubscriber, Decoupler};
use rx::processor::{MapVal1, Map, TraceWhile, Reduce};
use rx::sendable::{Sendable};
use rx::reactor::{StreamBuf, NetEngine, Reactor};
use rx::protocol::{BufProtocol};
use std::mem;
use std::str;
use std::io::Timer;
use std::time::Duration;
use std::fmt;
use time::precise_time_ns;
use std::sync::mpsc::channel;
use std::thread::Thread;
//static BUF_SIZE : uisize = 1600;

struct MyAllocator;


impl Allocator for MyAllocator {
    fn allocate(&self, size: usize, align: usize) -> *mut u8 {
        unsafe { a::allocate(size, align) }
    }

    fn deallocate(&self, ptr: *mut u8, len: usize, align: usize) {
        unsafe { a::deallocate(ptr, len, align) }
    }
}

lazy_static! {
    static ref MYALLOC: Arc<Box<Allocator>> = {
        Arc::new( (Box::new(MyAllocator) as Box<Allocator>))
    };
}

#[test]
fn main() {

    println!("You are here");

    let mut ne = NetEngine<BufProtocol::new(1500, 100, 100);

    let srv_rx = ne.listen("127.0.0.1", 10000).unwrap();
    let cli = ne.connect("127.0.0.1", 10000).unwrap();

    let token = cli.tok.clone();
    let dtx = cli.dtx.clone();

    ne.timeout(Duration::milliseconds(1000), Box::new(|&: el : &mut Reactor| { el.shutdown(); true}));

    let out = move |:| {
        let mut rec = Box::new(Coupler::new(srv_rx));
        let mut map1 = Box::new(Map::new(| tup : StreamBuf |  -> (StreamBuf, u64) {  (tup, precise_time_ns()) }));
        let mut red = Box::new(Reduce::new(0u64, | count, (tup, t) : (StreamBuf, u64) | -> (u64, (StreamBuf, u64, u64)) {let c = count + 1; (c, (tup, t, c)) }));
        let mut trace = Box::new(TraceWhile::new(| &(_,_,count) : &(StreamBuf, u64, u64) | { count % 10000 == 0 }));
        let mut map2 = Box::new(MapVal1::new(token, |(StreamBuf (buf, _),_,_), t: &Token| -> StreamBuf { StreamBuf (buf, *t) }));
        // change the token from t to the token of the connection, that way Decoupler
        // will tell eventloop to send it back out of the tcp connections's pipe
        let mut sen = Box::new(Decoupler::new(dtx));

        println!("before the thingy");
        map2.subscribe(sen);
        trace.subscribe(map2);
        red.subscribe(trace);
        map1.subscribe(red);
        rec.subscribe(map1);

        rec.run();
    };

    let guard = Thread::spawn(out);

    let mut buf = RWIobuf::from_str_copy_with_allocator("AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz012345678901", MYALLOC.clone());
    cli.dtx.send( StreamBuf (buf.atomic_read_only().unwrap(), token)).unwrap();
    // Start the event loop
    ne.run();
}
