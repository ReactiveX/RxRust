use std::ops::{Shl, Shr};
use std::cell::RefCell;

pub trait Subscriber {
    type Input;

    fn on_next(&mut self, t: Self::Input) -> bool;
    fn on_subscribe(&mut self, usize) {
        debug!("on_subscribe called");
    }
    fn on_error(&mut self, err: &str) {
        error!("on_error called: {:?}", err);
    }
    fn on_complete(&mut self, force: bool) {
        debug!("on_complete called");
    }
}

pub trait Publisher<'a> {
    type Output;
    fn subscribe<S>(&mut self, Box<S>) where S : Subscriber<Input=Self::Output>;

    fn next(&mut self) -> bool {
        panic!("Unimplemented fn, presumably run() or next() is being attempted on a processor, not a publisher");
    }

    fn run(&mut self) {
        loop {
            if ! self.next() {
                break
            }
        }
    }
}

pub trait Processor<'a> : Subscriber + Publisher<'a> { }

impl<'a, P> Processor<'a> for P
where P : Subscriber + Publisher<'a> { }
