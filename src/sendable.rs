// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.
//
use std::sync::mpsc::{SyncSender, SendError, Sender};
use mio::EventLoopSender;

/// Sendable
/// Wrapper trait for all types of queue senders
pub trait Sendable : Sized + Clone {
    type Item;
    fn send(&self, a: Self::Item) -> Result<(), Self::Item>;
}

impl<A : Send> Sendable for Sender<A> {
    type Item = A;
    fn send(&self, a: <Self as Sendable>::Item) -> Result<(), <Self as Sendable>::Item> {
        match self.send(a) {
            Ok(_) => Ok(()),
            Err(SendError(e))  => Err(e)
        }
    }
}

impl<A : Send> Sendable for SyncSender<A> {
    type Item = A;
    fn send(&self, a: <Self as Sendable>::Item) -> Result<(), <Self as Sendable>::Item> {
        match self.send(a) {
            Ok(_) => Ok(()),
            Err(SendError(e))  => Err(e)
        }
    }
}

impl<A : Send + Clone> Sendable for EventLoopSender<A> {
    type Item = A;
    fn send(&self, a: <Self as Sendable>::Item) -> Result<(), <Self as Sendable>::Item> {
        match self.send(a) {
            Ok(_) => Ok(()),
            e@Err(_)  => e
        }
    }
}

