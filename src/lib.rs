// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.
//! # Reactive Rust
//! A reactive streams library in the vein of [reactive-streams.org](http://reactive-streams.org)
//! with inspiration taken from [Elm](http://elm-lang.org)
//! Reactive streams are a mechanism for chaining computation in an event-driven way.
//! If it helps, think of them as Iterators but Push instead of Pull
//!
//! This library is still early alpha, and surely has bugs.
//!
//! ## Goals
//!
//! * Speed - Rust is a systems library. Convieniences
//!    are foresaken for performance where necessary
//! * 0-Copy - The core IO mechanisms pass around refcounted handles to buffers,
//!    data passing is typically move where possible, clone/copy is rarely required
//! * Extensibility - It is assumed that the user of the API will want their own
//!    objects to be easily made reactive.
//! * Familiarity - The "builder" API has a similar look and feel as the
//!     stdlibs Iterator API, but reactive streams themselves are much more
//!     loosely coupled
//!
//! The core abstraction of Reactive Streams is a [Publisher](reactive/trait.Publisher.html) /
//! [Subscriber](reactive/trait.Subscriber)
//!
//! Objects which are both a Publisher and a Subscriber are a [Processor](reactive/trait.Processor.html)
//!
//!
//! ## Example
//!
//! ```rust
//! ```
#![doc(html_root_url = "http://www.rust-ci.org/rrichardson/reactive/doc/reactive/")]
#![unstable]
#![crate_id = "rx"]
#![crate_type="lib"]

#![allow(unstable)]
#![feature(slicing_syntax)]
#![feature(unboxed_closures)]
#![feature(unsafe_destructor)]

extern crate core;
extern crate mio;
extern crate collections;
extern crate iobuf;
extern crate time;
extern crate rand;
extern crate nix;
extern crate libc;
extern crate quickcheck;

#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

#[macro_use]
pub mod default_macros;
pub mod subscriber;
pub mod reactive;
pub mod reactor;
pub mod net_stream;
pub mod sendable;
pub mod mmap_allocator;
mod processorimpl;
mod publisherimpl;
mod processorext;
mod publisherext;

pub mod publisher {
    pub use publisherimpl::*;
    pub use publisherext::*;
}

pub mod processor {
    pub use processorimpl::*;
    pub use processorext::*;
}

#[test]
fn main() {
    use publisher::{IterPublisher, Coupler};
    use processor::{Map};
    use subscriber::{StdoutSubscriber, Decoupler};
    use reactive::{Publisher, Subscriber};
    use std::io::Timer;
    use std::time::Duration;
    use std::sync::mpsc::{channel};
    use std::thread::Thread;
    let (dtx, drx) = channel();

    let out = move |:| {
        let sub = Box::new(StdoutSubscriber::new());
        let mut rec = Box::new(Coupler::new(drx));
        rec.subscribe(sub);
    };

    let gen = move |:| {
        let it = range(0is, 20is);
        let q   = Box::new(Decoupler::new(dtx.clone()));
        let mut map1 = Box::new(Map::new(|i : isize| {i * 10}));
        let mut map2 = Box::new(Map::new(|i : isize| {i + 2}));
        let mut iter = Box::new(IterPublisher::new(it));


        map2.subscribe(q);
        map1.subscribe(map2);
        iter.subscribe(map1);
    };

    Thread::spawn(out);
    Thread::spawn(gen);

    let mut timer = Timer::new().unwrap();
    timer.sleep(Duration::milliseconds(1000));

    Thread::spawn(|| {
        let it = range(0is, 20is);
        let sub = Box::new(StdoutSubscriber::new());
        let mut map1 = Box::new(Map::new(|i : isize| {i * 10}));
        let mut map2 = Box::new(Map::new(|i : isize| {i + 2}));
        let mut iter = Box::new(IterPublisher::new(it));


        map2.subscribe(sub);
        map1.subscribe(map2);
        iter.subscribe(map1);

    });


    timer.sleep(Duration::milliseconds(1000));

}
