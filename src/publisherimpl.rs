// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.

use reactive::{Publisher, Subscriber};

use quickcheck::{Arbitrary, Gen, StdGen};

use std::sync::mpsc::{Sender, Receiver, TryRecvError};
use std::rand::Rng;

use rand::isaac::Isaac64Rng as IRng;

pub struct TestIncGen {
    current: u64
}

impl Rng for TestIncGen {
    fn next_u32(&mut self) -> u32 {
        let c = self.current;
        self.current += 1;
        c as u32
    }

    fn next_u64(&mut self) -> u64 {
        let c = self.current;
        self.current += 1;
        c
    }
}

pub struct RndGen<'a, 'b, O> where O : Arbitrary {
    subscriber: Option<Box<Subscriber<Input=O> + 'a>>,
    gen: StdGen<TestIncGen>
}

impl<'a, 'b, O> RndGen<'a, 'b, O> where O : Arbitrary {
    pub fn new() -> RndGen<'a, 'b, O>
    {
        RndGen {
            subscriber: None,
            gen: StdGen::new(TestIncGen { current: 0 }, 1_000_000)
        }
    }

}

impl<'a,'b, O> Publisher<'a> for RndGen<'a, 'b, O> where  O : Arbitrary {
    type Output = O;
    fn subscribe(&mut self, s: Box<Subscriber<Input=O> + 'a>) {
        let s: Box<Subscriber<Input=O>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }

    fn try_next(&mut self) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => s.on_next(Arbitrary::arbitrary(&mut self.gen)),
            None => {error!("My subscriber went away");false}
        }
    }
}


//
// Iterator Publisher
//

pub struct IterPublisher<'a, 'b, 'c, O, Iter>
where Iter: Iterator + 'b , O : 'c
{
    iter: Iter,
    subscriber: Option<Box<Subscriber<Input=O> + 'a>>
}

impl<'a, 'b, 'c, O, Iter> IterPublisher<'a, 'b, 'c, O, Iter>
where Iter: Iterator<Item=O> + 'b ,
      O : 'c
{
    pub fn new(iter: Iter) -> IterPublisher<'a, 'b, 'c, O, Iter>
    {
        IterPublisher {
            iter: iter,
            subscriber: None
        }
    }


}

impl<'a, 'b, 'c, O, Iter> Publisher<'a> for IterPublisher<'a, 'b, 'c, O, Iter>
where Iter: Iterator<Item=O> + 'b,
      O : 'c
{

    type Output = O;
    fn subscribe(&mut self, s: Box<Subscriber<Input=O> + 'a>) {
        let s: Box<Subscriber<Input=O>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }

    fn next(&mut self) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => {
                match self.iter.next() {
                    Some(x) => s.on_next(x),
                    None => { s.on_complete(false); false }
                }
            },
            None => {error!("My subscriber went away");false}
        }
    }
}

//
// Coupler
//
pub struct Coupler<'a, O> where O : Send {
    data_rx: Receiver<O>,
    subscriber: Option<Box<Subscriber<Input=O> + 'a>>
}


impl<'a, O> Coupler<'a, O> where O : Send {
    pub fn new(rx: Receiver<O>) -> Coupler<'a, O> {
        Coupler {
            data_rx: rx,
            subscriber: None,
        }
    }

}


impl<'a, O> Publisher<'a> for Coupler<'a, O> where O : Send {

    type Output = O;
    fn subscribe(&mut self, s: Box<Subscriber<Input=O> + 'a>) {
        let s: Box<Subscriber<Input=O>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }

    // Does not block
    fn next (&mut self) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => match self.data_rx.recv() {
                Ok(d) => s.on_next(d),
                Err(..) => { info!("The other end of the coupler queue went away"); s.on_complete(false); false }
            },
            None => { error!("My subscriber went away"); false }
        }
    }

    fn try_next(&mut self) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => match self.data_rx.try_recv() {
                Ok(d) => s.on_next(d),
                Err(TryRecvError::Empty) => true,
                Err(TryRecvError::Disconnected) => { 
                    info!("The other end of the coupler queue went away"); s.on_complete(false); false 
                }
            },
            None => { error!("My subscriber went away"); false }
        }
    }
}

//
// Repeat
//
pub struct Repeat<'a, O> where O : Send + Clone {
    val: O,
    subscriber: Option<Box<Subscriber<Input=O> + 'a>>
}


impl<'a, O> Repeat<'a, O> where O : Send + Clone {
    pub fn new(o: O) -> Repeat<'a, O> {
        Repeat {
            val: o,
            subscriber: None,
        }
    }

}


impl<'a, O> Publisher<'a> for Repeat<'a, O> where O : Send + Clone {
    type Output = O;
    fn subscribe(&mut self, s: Box<Subscriber<Input=O> + 'a>) {
        let s: Box<Subscriber<Input=O>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }

    fn next (&mut self) -> bool{
        match self.subscriber.as_mut() {
            Some(s) => s.on_next(self.val.clone()),
            None => { error!("My subscriber went away"); false }
        }
    }
}


