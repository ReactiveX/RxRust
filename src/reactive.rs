// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.



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

    fn subscribe(&mut self, Box<Subscriber<Input=Self::Output> + 'a>);

    /// The basic message event generation function
    /// this is typically called in a loop
    /// This version of the function can block
    fn next(&mut self) -> bool {
        self.try_next()
    }

    /// The basic message event generation function
    /// this is typically called in a loop
    /// It is expected that this next will never block 
    fn try_next(&mut self) -> bool {
        panic!("Unimplemented fn, presumably run() or next() is being attempted on a processor, not a publisher");
    }

    /// Runs in a loop, expects that its publisher might block
    fn run(&mut self) {
        debug!("Starting loop");
        loop {
            if ! self.next() {
                break
            }
        }
        debug!("Done with loop");
    }
   
}

pub trait Processor<'a> : Subscriber + Publisher<'a> { }

impl<'a, P> Processor<'a> for P
where P : Subscriber + Publisher<'a> { }
