use std::future::Future;

use crate::io::AsyncReadRent;

/// AsyncBufRead: async read with buffered content
pub trait AsyncBufRead: AsyncReadRent {
    /// The returned future of fill_buf
    type FillBufFuture<'a>: Future<Output = std::io::Result<&'a [u8]>>
    where
        Self: 'a;

    /// Try read data and get a reference to the internal buffer
    fn fill_buf(&mut self) -> Self::FillBufFuture<'_>;
    /// Mark how much data is read
    fn consume(&mut self, amt: usize);
}
