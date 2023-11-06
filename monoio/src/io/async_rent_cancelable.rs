use std::future::Future;

use super::{AsyncReadRent, AsyncWriteRent, CancelHandle};
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    BufResult,
};

/// CancelableAsyncReadRent: async read with a ownership of a buffer and ability to cancel io.
pub trait CancelableAsyncReadRent: AsyncReadRent {
    /// Same as read(2)
    fn cancelable_read<T: IoBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>>;
    /// Same as readv(2)
    fn cancelable_readv<T: IoVecBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>>;
}

impl<A: ?Sized + CancelableAsyncReadRent> CancelableAsyncReadRent for &mut A {
    #[inline]
    fn cancelable_read<T: IoBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>> {
        (**self).cancelable_read(buf, c)
    }

    #[inline]
    fn cancelable_readv<T: IoVecBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>> {
        (**self).cancelable_readv(buf, c)
    }
}

/// CancelableAsyncWriteRent: async write with a ownership of a buffer and ability to cancel io.
pub trait CancelableAsyncWriteRent: AsyncWriteRent {
    /// Same as write(2)
    fn cancelable_write<T: IoBuf>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>>;

    /// Same as writev(2)
    fn cancelable_writev<T: IoVecBuf>(
        &mut self,
        buf_vec: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>>;

    /// Flush buffered data if needed
    fn cancelable_flush(&mut self, c: CancelHandle) -> impl Future<Output = std::io::Result<()>>;

    /// Same as shutdown
    fn cancelable_shutdown(&mut self, c: CancelHandle)
        -> impl Future<Output = std::io::Result<()>>;
}

impl<A: ?Sized + CancelableAsyncWriteRent> CancelableAsyncWriteRent for &mut A {
    #[inline]
    fn cancelable_write<T: IoBuf>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>> {
        (**self).cancelable_write(buf, c)
    }

    #[inline]
    fn cancelable_writev<T: IoVecBuf>(
        &mut self,
        buf_vec: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>> {
        (**self).cancelable_writev(buf_vec, c)
    }

    #[inline]
    fn cancelable_flush(&mut self, c: CancelHandle) -> impl Future<Output = std::io::Result<()>> {
        (**self).cancelable_flush(c)
    }

    #[inline]
    fn cancelable_shutdown(
        &mut self,
        c: CancelHandle,
    ) -> impl Future<Output = std::io::Result<()>> {
        (**self).cancelable_shutdown(c)
    }
}
