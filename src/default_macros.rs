// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.


#[macro_export]
macro_rules! default_pass_error(
    () => (
        fn on_error(&mut self, err: &str) {
            error!("Error: {:?}", err);
            match self.subscriber.as_mut() {
                Some(s) =>  s.on_error(err),
                None => {panic!("on_error called but I don't have a subscriber")}
            }
        }
    )
);

#[macro_export]
macro_rules! default_pass_complete (
    () => (
        fn on_complete(&mut self, force: bool) {
            match self.subscriber.as_mut() {
                Some(s) => s.on_complete(force),
                None => panic!("on_complete called but I don't have a subscriber")
            }
        }
    )
);

#[macro_export]
macro_rules! default_pass_subscribe (
    () => (
        fn on_subscribe(&mut self, index: usize) {
            self.index = Some(index);
        }
    )
);
