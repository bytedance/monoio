use std::future::Future;

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf},
    BufResult,
};

/// AsyncWriteRent: async write with a ownership of a buffer
pub trait AsyncWriteRent {
    /// The future of write Result<size, buffer>
    type WriteFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;
    /// The future of writev Result<size, buffer>
    type WritevFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// The future of shutdown
    type ShutdownFuture<'a>: Future<Output = Result<(), std::io::Error>>
    where
        Self: 'a;

    /// Same as write(2)
    fn write<T: IoBuf>(&self, buf: T) -> Self::WriteFuture<'_, T>;

    /// Same as writev(2)
    fn writev<T: IoVecBuf>(&self, buf_vec: T) -> Self::WritevFuture<'_, T>;

    /// Same as shutdown
    fn shutdown(&self) -> Self::ShutdownFuture<'_>;
}

/// AsyncWriteRentAt: async write with a ownership of a buffer and a position
pub trait AsyncWriteRentAt {
    /// The future of Result<size, buffer>
    type Future<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Write buf at given offset
    fn write_at<T: IoBufMut>(&self, buf: T, pos: usize) -> Self::Future<'_, T>;
}
