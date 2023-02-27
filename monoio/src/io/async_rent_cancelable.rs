use std::future::Future;

use super::{AsyncReadRent, AsyncWriteRent, CancelHandle};
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    BufResult,
};

/// CancelableAsyncReadRent: async read with a ownership of a buffer and ability to cancel io.
pub trait CancelableAsyncReadRent: AsyncReadRent {
    /// The future of read Result<size, buffer>
    type CancelableReadFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoBufMut + 'a;
    /// The future of readv Result<size, buffer>
    type CancelableReadvFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoVecBufMut + 'a;

    /// Same as read(2)
    fn cancelable_read<T: IoBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadFuture<'_, T>;
    /// Same as readv(2)
    fn cancelable_readv<T: IoVecBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadvFuture<'_, T>;
}

impl<A: ?Sized + CancelableAsyncReadRent> CancelableAsyncReadRent for &mut A {
    type CancelableReadFuture<'a, T> = A::CancelableReadFuture<'a, T>
    where
        Self: 'a,
        T: IoBufMut + 'a;

    type CancelableReadvFuture<'a, T> = A::CancelableReadvFuture<'a, T>
    where
        Self: 'a,
        T: IoVecBufMut + 'a;

    #[inline]
    fn cancelable_read<T: IoBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadFuture<'_, T> {
        (**self).cancelable_read(buf, c)
    }

    #[inline]
    fn cancelable_readv<T: IoVecBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadvFuture<'_, T> {
        (**self).cancelable_readv(buf, c)
    }
}

/// CancelableAsyncWriteRent: async write with a ownership of a buffer and ability to cancel io.
pub trait CancelableAsyncWriteRent: AsyncWriteRent {
    /// The future of write Result<size, buffer>
    type CancelableWriteFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoBuf + 'a;
    /// The future of writev Result<size, buffer>
    type CancelableWritevFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoVecBuf + 'a;

    /// The future of flush
    type CancelableFlushFuture<'a>: Future<Output = std::io::Result<()>>
    where
        Self: 'a;

    /// The future of shutdown
    type CancelableShutdownFuture<'a>: Future<Output = std::io::Result<()>>
    where
        Self: 'a;

    /// Same as write(2)
    fn cancelable_write<T: IoBuf>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableWriteFuture<'_, T>;

    /// Same as writev(2)
    fn cancelable_writev<T: IoVecBuf>(
        &mut self,
        buf_vec: T,
        c: CancelHandle,
    ) -> Self::CancelableWritevFuture<'_, T>;

    /// Flush buffered data if needed
    fn cancelable_flush(&mut self, c: CancelHandle) -> Self::CancelableFlushFuture<'_>;

    /// Same as shutdown
    fn cancelable_shutdown(&mut self, c: CancelHandle) -> Self::CancelableShutdownFuture<'_>;
}

impl<A: ?Sized + CancelableAsyncWriteRent> CancelableAsyncWriteRent for &mut A {
    type CancelableWriteFuture<'a, T> = A::CancelableWriteFuture<'a, T>
    where
        Self: 'a,
        T: IoBuf + 'a;

    type CancelableWritevFuture<'a, T> = A::CancelableWritevFuture<'a, T>
    where
        Self: 'a,
        T: IoVecBuf + 'a;

    type CancelableFlushFuture<'a> = A::CancelableFlushFuture<'a>
    where
        Self: 'a;

    type CancelableShutdownFuture<'a> = A::CancelableShutdownFuture<'a>
    where
        Self: 'a;

    #[inline]
    fn cancelable_write<T: IoBuf>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableWriteFuture<'_, T> {
        (**self).cancelable_write(buf, c)
    }

    #[inline]
    fn cancelable_writev<T: IoVecBuf>(
        &mut self,
        buf_vec: T,
        c: CancelHandle,
    ) -> Self::CancelableWritevFuture<'_, T> {
        (**self).cancelable_writev(buf_vec, c)
    }

    #[inline]
    fn cancelable_flush(&mut self, c: CancelHandle) -> Self::CancelableFlushFuture<'_> {
        (**self).cancelable_flush(c)
    }

    #[inline]
    fn cancelable_shutdown(&mut self, c: CancelHandle) -> Self::CancelableShutdownFuture<'_> {
        (**self).cancelable_shutdown(c)
    }
}
