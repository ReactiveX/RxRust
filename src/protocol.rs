
use iobuf::{AROIobuf, Iobuf};

pub trait Protocol {
    type Output : Send + 'static;

    fn new() -> Self;

    fn append(&mut self, &AROIobuf) -> Option<(Self::Output, AROIobuf, u32)>;
}


pub struct BufProtocol;

impl Protocol for BufProtocol {
    type Output = AROIobuf;

    fn new() -> BufProtocol {
        BufProtocol
    }

    fn append(&mut self, buf: &AROIobuf) -> Option<(<Self as Protocol>::Output, AROIobuf, u32)> {
        if buf.len() >= 64 {
            let (a, b) = buf.split_at(64).unwrap();
            Some((a, b, 64))
        } else {
            None
        }
    }
}
