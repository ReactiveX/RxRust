// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.

use std::fmt::Display;
use reactive::{Subscriber};
use sendable::Sendable;

pub struct StdoutSubscriber<A> where A : Display {
    index: Option<usize>
}

impl<A> StdoutSubscriber<A> where A : Display {

    pub fn new() -> StdoutSubscriber<A> {
        StdoutSubscriber {
            index: None
        }
    }
}

impl<A> Subscriber for StdoutSubscriber<A> where A : Display {
    type Input = A;

    fn on_next(&mut self, t: A) -> bool {
        println!("{}", t);
        true
    }
}

pub struct Decoupler<Q, I> where I : Send, Q : Sendable {
    index: Option<usize>,
    data_tx: Q,
}

impl<Q, I> Decoupler<Q, I> where I : Send, Q : Sendable {

    pub fn new(tx: Q) -> Decoupler<Q,I> {
        Decoupler {
            index: None,
            data_tx: tx,
        }
    }
}

impl<Q, I> Subscriber for Decoupler<Q, I>
where I : Send,
      Q : Sendable<Item=I>
{
    type Input = I;

    fn on_next(&mut self, t: I) -> bool {
        //TODO better handle queue failure, maybe put the returned buf
        //isizeo a recovery queue
        match self.data_tx.send(t) {
            Ok(()) => true,
            Err(_) => false
        }
    }
}

pub struct Collect<'a, I> where I : 'a {
    index: Option<usize>,
    val: &'a mut Box<Vec<I>>
}

impl<'a, I> Collect<'a, I> where I : 'a {

    pub fn new(v : &'a mut Box<Vec<I>>) -> Collect<'a, I> {
        Collect {
            index: None,
            val: v
        }
    }
}

impl<'a, I> Subscriber for Collect<'a, I> where I : 'a {
    type Input = I;

    fn on_next(&mut self, t: I) -> bool {
        self.val.push(t);
        true
    }
}

