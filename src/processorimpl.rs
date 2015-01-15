// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.
//

use std::fmt::Show;
use reactive::{Publisher, Subscriber};
use sendable::Sendable;

pub struct Trace<'a, I> where I : Show {
    subscriber: Option<Box<Subscriber<Input=I> + 'a>>,
    index: Option<usize>
}

impl<'a, I> Trace<'a, I> where I : Show {

    pub fn new() -> Trace<'a, I>
    {
        Trace {
            subscriber: None,
            index: None
        }
    }
}

impl<'a, I> Publisher<'a> for Trace<'a, I> where I : Show {
    type Output = I;
    fn subscribe(&mut self, s: Box<Subscriber<Input=I> + 'a>) {
        let t: Box<Subscriber<Input=I>+'a> = s;
        self.subscriber = Some(t);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }
}

impl<'a, I> Subscriber for Trace<'a, I> where I : Show {
    type Input = I;
    fn on_next(&mut self, t: I) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => { println!("{:?}", t); s.on_next(t) },
            None => {true}
        }
    }

    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();
}

pub struct TraceWhile<'a, I, F> where I : Show, F : Fn(&I) -> bool {
    fun: F,
    subscriber: Option<Box<Subscriber<Input=I> + 'a>>,
    index: Option<usize>
}


impl<'a, I, F> TraceWhile<'a, I, F> where I : 'a + Show, F : Fn(&I) -> bool{

    pub fn new( f: F ) -> TraceWhile<'a, I, F>
    {
        TraceWhile {
            fun: f,
            subscriber: None,
            index: None
        }
    }
}

impl<'a, I, F> Publisher<'a> for TraceWhile<'a, I, F> where I : Show, F : Fn(&I) -> bool  {
    type Output = I;
    fn subscribe(&mut self, s: Box<Subscriber<Input=I> + 'a>) {
        let s: Box<Subscriber<Input=I>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }
}

impl<'a, I, F> Subscriber for TraceWhile<'a, I, F> where I : Show, F : Fn(&I) -> bool {
    type Input = I;

    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();

    fn on_next(&mut self, t: I) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => { if (self.fun)(&t) { println!("{:?}", t) }
                         s.on_next(t) }
            None => {true}
        }
    }
}

/// Do
/// Applies a () function at each iteration to the supplied value
/// which does not effect the result passed to the subscriber

pub struct Do<'a, I, F> where F : Fn(&I) -> () {
    fun: F,
    subscriber: Option<Box<Subscriber<Input=I> + 'a>>,
    index: Option<usize>
}


impl<'a, I, F> Do<'a, I, F> where F : Fn(&I) -> () {

    pub fn new(f: F) -> Do<'a, I, F>
    {
        Do {
            fun: f,
            subscriber: None,
            index: None
        }
    }
}

impl<'a, I, F> Publisher<'a> for Do<'a, I, F> where F : Fn(&I) -> () {
    type Output = I;
    fn subscribe(&mut self, s: Box<Subscriber<Input=I> + 'a>) {
        let s: Box<Subscriber<Input=I>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }
}

impl<'a, I, F> Subscriber for Do<'a, I, F> where F : Fn(&I) -> () {
    type Input = I;

    fn on_next(&mut self, t: I) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => { (self.fun)(&t); s.on_next(t) }
            None => {true}
        }
    }

    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();
}



/// Map
///
///

pub struct Map<'a, I, O, F> where F : Fn(I) -> O {
    fun: F,
    subscriber: Option<Box<Subscriber<Input=O> + 'a>>,
    index: Option<usize>
}


impl<'a, I, O, F> Map<'a, I, O, F> where F : Fn(I) -> O {

    pub fn new(f: F) -> Map<'a, I, O, F>
    {
        Map {
            fun: f,
            subscriber: None,
            index: None
        }
    }
}

impl<'a, I, O, F> Publisher<'a> for Map<'a, I, O, F> where F : Fn(I) -> O {
    type Output = O;
    fn subscribe(&mut self, s: Box<Subscriber<Input=O> + 'a>) {
        let s: Box<Subscriber<Input=O>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
    }
}

impl<'a, I, O, F> Subscriber for Map<'a, I, O, F> where F : Fn(I) -> O {
    type Input = I;

    fn on_next(&mut self, t: I) -> bool {
        match self.subscriber.as_mut() {
            Some(s) => s.on_next((self.fun)(t)),
            None => {true}
        }
    }

    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();
}


pub struct MapVal1<'a, 'c, I, V, O, F> where O : 'c, V : Clone, F : Fn(I,&V) -> O {
    fun: F,
    subscriber: Option<Box<Subscriber<Input=O> + 'a>>,
    index: Option<usize>,
    val: V
}


impl<'a, 'c, I, V, O, F> MapVal1<'a, 'c, I, V, O, F> where O : 'c, V : Clone, F : Fn(I,&V) -> O {

    pub fn new( v : V, f: F ) -> MapVal1<'a, 'c, I, V, O, F>
    {
        MapVal1 {
            fun: f,
            subscriber: None,
            index: None,
            val: v.clone()
        }
    }
}

impl<'a, 'c, I, V, O, F> Publisher<'a> for MapVal1<'a, 'c, I, V, O, F> where O : 'c, V : Clone, F : Fn(I,&V) -> O {
    type Output = O;

    fn subscribe(&mut self, s: Box<Subscriber<Input=O> + 'a>) {
        let s: Box<Subscriber<Input=O>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
     }
}

impl<'a, 'c, I, V, O, F> Subscriber for MapVal1<'a, 'c, I, V, O, F> where O : 'c, V : Clone, F : Fn(I,&V) -> O {
    type Input = I;

    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();

    fn on_next(&mut self, t: I) -> bool {
        match self.subscriber.as_mut() {
            Some(s) =>  s.on_next((self.fun)(t, &self.val)),
            None => {true}
        }
    }

}


//
// Reduce
//

pub struct Reduce<'a, 'c, I, V, O, F> where O : 'c, V : Copy, F : Fn(V,I) -> (V, O) {
    fun: F,
    subscriber: Option<Box<Subscriber<Input=O> + 'a>>,
    index: Option<usize>,
    state: V
}


impl<'a, 'c, I, V, O, F> Reduce<'a, 'c, I, V, O, F> where O : 'c, V : Copy, F : Fn(V,I) -> (V, O) {

    pub fn new( initial : V, f: F ) -> Reduce<'a, 'c, I, V, O, F>
    {
        Reduce {
            fun: f,
            subscriber: None,
            index: None,
            state: initial
        }
    }
}

impl<'a, 'c, I, V, O, F> Publisher<'a> for Reduce<'a, 'c, I, V, O, F> where O : 'c, V : Copy, F : Fn(V,I) -> (V, O) {
    type Output = O;

    fn subscribe(&mut self, s: Box<Subscriber<Input=O> + 'a>) {
        let s: Box<Subscriber<Input=O>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
     }
}

impl<'a, 'c, I, V, O, F> Subscriber for Reduce<'a, 'c, I, V, O, F> where O : 'c, V : Copy, F : Fn(V,I) -> (V, O) {
    type Input = I;

    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();

    fn on_next(&mut self, t: I) -> bool {
        let (newstate, outval) = (self.fun)(self.state, t);
        self.state = newstate;
        match self.subscriber.as_mut() {
            Some(s) =>  { s.on_next(outval) }
            None => {true}
        }
    }
}

//
// Enumerate
//
//

pub struct Enumerate<'a, I> {
    subscriber: Option<Box<Subscriber<Input=(I, u64)> + 'a>>,
    index: Option<usize>,
    count: u64
}


impl<'a, I> Enumerate<'a, I> {

    pub fn new() -> Enumerate<'a, I>
    {
        Enumerate {
            subscriber: None,
            index: None,
            count: 0
        }
    }
}

impl<'a, I> Publisher<'a> for Enumerate<'a, I> {
    type Output = (I, u64);

    fn subscribe(&mut self, s: Box<Subscriber<Input=(I, u64)> + 'a>) {
        let s: Box<Subscriber<Input=(I, u64)>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
     }
}

impl<'a, I> Subscriber for Enumerate<'a, I> {
    type Input = I;

    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();

    fn on_next(&mut self, t: I) -> bool {
        match self.subscriber.as_mut() {
            Some(s) =>  { let c = self.count;
                          self.count += 1;
                          s.on_next( (t, c)  ) }
            None => {true}
        }
    }
}

/// Tee
/// Sends the value off to a supplied Sendable queue,
/// then passes the data to its subscriber
///
pub struct Tee<'a, Q, I>
where I : Send + Clone,
Q : Sendable
{

    subscriber: Option<Box<Subscriber<Input=I> + 'a>>,
    index: Option<usize>,
    data_tx: Q,
}

impl<'a, Q, I> Tee<'a, Q, I>
where I : Send + Clone,
Q : Sendable
{

    pub fn new(tx: Q) -> Tee<'a, Q,I> {
        Tee {
            index: None,
            subscriber: None,
            data_tx: tx,
        }
    }
}

impl<'a, Q, I> Publisher<'a> for Tee<'a, Q, I>
where I : Send + Clone,
Q : Sendable<Item=I> {

    type Output = I;

    fn subscribe(&mut self, s: Box<Subscriber<Input=I> + 'a>) {
        let s: Box<Subscriber<Input=I>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
     }
}

impl<'a, Q, I> Subscriber for Tee<'a, Q, I>
where I : Send + Clone,
Q : Sendable<Item=I> {

    type Input = I;

    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();

    fn on_next(&mut self, t: I) -> bool {
        match self.subscriber.as_mut() {
            Some(s) =>  { self.data_tx.send(t.clone()); s.on_next(t) }
            None => {true}
        }
    }
}



/// Unzip
/// takes tuples of identical items as input
/// unpacks them into their own message
///
pub struct Unzip<'a, I>
{
    subscriber: Option<Box<Subscriber<Input=I> + 'a>>,
    index: Option<usize>,
}

impl<'a, I> Unzip<'a, I> {
    pub fn new() -> Unzip<'a, I> {
        Unzip {
            index: None,
            subscriber: None,
        }
    }
}

impl<'a, I> Publisher<'a> for Unzip<'a, I>
{
    type Output = I;

    fn subscribe(&mut self, s: Box<Subscriber<Input=I> + 'a>) {
        let s: Box<Subscriber<Input=I>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
     }
}

impl<'a, I> Subscriber for Unzip<'a, I>
{
    type Input = (I,I);
    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();

    fn on_next(&mut self, t: (I,I)) -> bool {
        match self.subscriber.as_mut() {
            Some(s) =>  {
                s.on_next(t.0) | s.on_next(t.1)
            }
            None => {true}
        }
    }
}


/// Take
/// takes tuples of identical items as input
/// unpacks them into their own message
///
pub struct Take<'a, O>
{
    subscriber: Option<Box<Subscriber<Input=O> + 'a>>,
    index: Option<usize>,
    count: usize,
    max: usize,
    notified: bool
}

impl<'a, O> Take<'a, O> {
    pub fn new(max: usize) -> Take<'a, O> {
        Take {
            index: None,
            subscriber: None,
            count: 0,
            max: max,
            notified: false
        }
    }
}

impl<'a, O> Publisher<'a> for Take<'a, O>
{
    type Output = O;

    fn subscribe(&mut self, s: Box<Subscriber<Input=O> + 'a>) {
        let s: Box<Subscriber<Input=O>+'a> = s;
        self.subscriber = Some(s);
        self.subscriber.as_mut().unwrap().on_subscribe(0);
     }
}

impl<'a, O> Subscriber for Take<'a, O>
{
    type Input = O;
    default_pass_subscribe!();
    default_pass_complete!();
    default_pass_error!();

    fn on_next(&mut self, t: O) -> bool {
        match self.subscriber.as_mut() {
            Some(s) =>  {
                self.count += 1;
                if self.count > self.max { s.on_complete(false); self.notified = true; false }
                else { s.on_next(t) }
            },
            None => {true}
        }
    }
}


