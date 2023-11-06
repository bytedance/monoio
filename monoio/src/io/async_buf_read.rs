use std::future::Future;

use crate::io::AsyncReadRent;

/// AsyncBufRead: async read with buffered content
pub trait AsyncBufRead: AsyncReadRent {
    /// Try read data and get a reference to the internal buffer
    fn fill_buf(&mut self) -> impl Future<Output = std::io::Result<&[u8]>>;
    /// Mark how much data is read
    fn consume(&mut self, amt: usize);
}
