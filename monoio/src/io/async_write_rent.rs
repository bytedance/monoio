use std::future::Future;

use crate::{
    buf::{IoBuf, IoVecBuf},
    BufResult,
};

/// AsyncWriteRent: async write with a ownership of a buffer
pub trait AsyncWriteRent {
    /// Same as write(2)
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>>;

    /// Same as writev(2)
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>>;

    /// Flush buffered data if needed
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>>;

    /// Same as shutdown
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>>;
}

/// AsyncWriteRentAt: async write with a ownership of a buffer and a position
pub trait AsyncWriteRentAt {
    /// Write buf at given offset
    fn write_at<T: IoBuf>(
        &mut self,
        buf: T,
        pos: usize,
    ) -> impl Future<Output = BufResult<usize, T>>;
}

impl<A: ?Sized + AsyncWriteRent> AsyncWriteRent for &mut A {
    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        (**self).write(buf)
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        (**self).writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>> {
        (**self).flush()
    }

    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>> {
        (**self).shutdown()
    }
}
