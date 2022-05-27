use std::future::Future;

use crate::{
    buf::{IoBufMut, IoVecBufMut},
    BufResult,
};

/// AsyncReadRent: async read with a ownership of a buffer
pub trait AsyncReadRent {
    /// The future of read Result<size, buffer>
    type ReadFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;
    /// The future of readv Result<size, buffer>
    type ReadvFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Same as read(2)
    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T>;
    /// Same as readv(2)
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T>;
}

/// AsyncReadRentAt: async read with a ownership of a buffer and a position
pub trait AsyncReadRentAt {
    /// The future of Result<size, buffer>
    type Future<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Same as Read(2)
    fn read_at<T: IoBufMut>(&mut self, buf: T, pos: usize) -> Self::Future<'_, T>;
}

impl<A: ?Sized + AsyncReadRent> AsyncReadRent for &mut A {
    type ReadFuture<'a, T> = A::ReadFuture<'a, T>
    where
        Self: 'a,
        T: 'a;

    type ReadvFuture<'a, T> = A::ReadvFuture<'a, T>
    where
        Self: 'a,
        T: 'a;

    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        (&mut **self).read(buf)
    }

    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        (&mut **self).readv(buf)
    }
}
