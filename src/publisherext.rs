// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.
use reactive::{Publisher, Subscriber, Processor};
use processorimpl::*;
use std::sync::Arc;
use std::cell::RefCell;

/*
pub trait PublisherExt<'p> : Publisher<'p> + Sized {

    fn map<'a, O, F>(mut self, f: F) -> PublisherWrapper<'a, Self, O>
    where F : Fn(Self::Output) -> O
    {
        let mut m : Box<Processor<'a, Input=Self::Output, Output=O> + 'a> = Box::new(Map::new(f));
        let t = Arc::new(RefCell::new(m));
        self.subscribe(*t.borrow_mut());
        PublisherWrapper { head: self, tail: t.clone()  }
    }
}

impl<'a, P> PublisherExt<'a> for P where P : Publisher<'a> {}

pub struct PublisherWrapper<'a, P, O>
where P : Publisher<'a> + 'a
{
    head: P,
    tail: Arc<RefCell<Box<Processor<'a> + 'a>>>
}

*/
