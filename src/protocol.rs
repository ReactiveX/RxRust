
use iobuf::{AROIobuf, Iobuf};

pub trait Protocol {
    type Output : Send + 'static;

    fn new() -> Self;

    fn append(&mut self, &AROIobuf) -> Option<(Self::Output, AROIobuf, u32)>;
}

pub trait HasSize{ fn size() -> u32; }

/// A simple Protocol implementation which produces buffers of a fixed size
#[derive(Debug)]
pub struct BufProtocol<S : HasSize>;


impl<S : HasSize> Protocol for BufProtocol<S> {
    type Output = AROIobuf;

    fn new() -> BufProtocol<S> {
        BufProtocol
    }

    fn append(&mut self, buf: &AROIobuf) -> Option<(<Self as Protocol>::Output, AROIobuf, u32)> {
        let bufsz = <S as HasSize>::size();
        if buf.len() >= bufsz {
            let (a, b) = buf.split_at(bufsz).unwrap();
            Some((a, b, bufsz))
        } else {
            None
        }
    }
}
