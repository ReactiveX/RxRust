
use iobuf::{AppendBuf, AROIobuf, Iobuf};

trait Protocol {
    type Output;

    fn new() -> Self;

    fn append(&mut self, &AROIobuf) -> Option<(Self::Output, usize)>;
}


struct BufProtocol;

impl Protocol for BufProtocol {
    type Output = AROIobuf;

    fn new() -> BufProtocol {
        BufProtocol
    }

    fn append(&mut self, buf: &AROIobuf) -> Option<(<Self as Protocol>::Output, usize)> {
        if buf.len() >= 64 {
            Some((buf.split_at(64).unwrap().0, 64))
        } else {
            None
        }
    }
}
