use std::{
    cell::UnsafeCell,
    error::Error,
    fmt::{self, Debug},
    future::Future,
    rc::Rc,
};

use super::CancelHandle;
use crate::{
    io::{AsyncReadRent, AsyncWriteRent, CancelableAsyncReadRent, CancelableAsyncWriteRent},
    BufResult,
};

/// Owned Read Half Part
#[derive(Debug)]
pub struct OwnedReadHalf<T>(pub Rc<UnsafeCell<T>>);
/// Owned Write Half Part
#[derive(Debug)]
#[repr(transparent)]
pub struct OwnedWriteHalf<T>(pub Rc<UnsafeCell<T>>)
where
    T: AsyncWriteRent;

/// This is a dummy unsafe trait to inform monoio,
/// the object with has this `Split` trait can be safely split
/// to read/write object in both form of `Owned` or `Borrowed`.
///
/// # Safety
///
/// monoio cannot guarantee whether the custom object can be
/// safely split to divided objects. Users should ensure the read
/// operations are indenpendence from the write ones, the methods
/// from `AsyncReadRent` and `AsyncWriteRent` can execute concurrently.
pub unsafe trait Split {}

/// Inner split trait
pub trait Splitable {
    /// Owned Read Split
    type OwnedRead;
    /// Owned Write Split
    type OwnedWrite;

    /// Split into owned parts
    fn into_split(self) -> (Self::OwnedRead, Self::OwnedWrite);
}

impl<T> Splitable for T
where
    T: Split + AsyncWriteRent,
{
    type OwnedRead = OwnedReadHalf<T>;
    type OwnedWrite = OwnedWriteHalf<T>;

    #[inline]
    fn into_split(self) -> (Self::OwnedRead, Self::OwnedWrite) {
        let shared = Rc::new(UnsafeCell::new(self));
        (OwnedReadHalf(shared.clone()), OwnedWriteHalf(shared))
    }
}

impl<Inner> AsyncReadRent for OwnedReadHalf<Inner>
where
    Inner: AsyncReadRent,
{
    #[inline]
    fn read<T: crate::buf::IoBufMut>(
        &mut self,
        buf: T,
    ) -> impl Future<Output = BufResult<usize, T>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.read(buf)
    }

    #[inline]
    fn readv<T: crate::buf::IoVecBufMut>(
        &mut self,
        buf: T,
    ) -> impl Future<Output = BufResult<usize, T>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.readv(buf)
    }
}

impl<Inner> CancelableAsyncReadRent for OwnedReadHalf<Inner>
where
    Inner: CancelableAsyncReadRent,
{
    #[inline]
    fn cancelable_read<T: crate::buf::IoBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = crate::BufResult<usize, T>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.cancelable_read(buf, c)
    }

    #[inline]
    fn cancelable_readv<T: crate::buf::IoVecBufMut>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = crate::BufResult<usize, T>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.cancelable_readv(buf, c)
    }
}

impl<Inner> AsyncWriteRent for OwnedWriteHalf<Inner>
where
    Inner: AsyncWriteRent,
{
    #[inline]
    fn write<T: crate::buf::IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.write(buf)
    }

    #[inline]
    fn writev<T: crate::buf::IoVecBuf>(
        &mut self,
        buf_vec: T,
    ) -> impl Future<Output = BufResult<usize, T>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.flush()
    }

    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.shutdown()
    }
}

impl<Inner> CancelableAsyncWriteRent for OwnedWriteHalf<Inner>
where
    Inner: CancelableAsyncWriteRent,
{
    #[inline]
    fn cancelable_write<T: crate::buf::IoBuf>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = crate::BufResult<usize, T>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.cancelable_write(buf, c)
    }

    #[inline]
    fn cancelable_writev<T: crate::buf::IoVecBuf>(
        &mut self,
        buf_vec: T,
        c: CancelHandle,
    ) -> impl Future<Output = crate::BufResult<usize, T>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.cancelable_writev(buf_vec, c)
    }

    #[inline]
    fn cancelable_flush(&mut self, c: CancelHandle) -> impl Future<Output = std::io::Result<()>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.cancelable_flush(c)
    }

    #[inline]
    fn cancelable_shutdown(
        &mut self,
        c: CancelHandle,
    ) -> impl Future<Output = std::io::Result<()>> {
        let stream = unsafe { &mut *self.0.get() };
        stream.cancelable_shutdown(c)
    }
}

impl<T> OwnedReadHalf<T>
where
    T: AsyncWriteRent,
{
    /// reunite write half
    #[inline]
    pub fn reunite(self, other: OwnedWriteHalf<T>) -> Result<T, ReuniteError<T>> {
        reunite(self, other)
    }
}

impl<T> OwnedWriteHalf<T>
where
    T: AsyncWriteRent,
{
    /// reunite read half
    #[inline]
    pub fn reunite(self, other: OwnedReadHalf<T>) -> Result<T, ReuniteError<T>> {
        reunite(other, self)
    }
}

impl<T> Drop for OwnedWriteHalf<T>
where
    T: AsyncWriteRent,
{
    #[inline]
    fn drop(&mut self) {
        let write = unsafe { &mut *self.0.get() };
        // Notes:: shutdown is an async function but rust currently does not support async drop
        // this drop will only execute sync part of `shutdown` function.
        #[allow(unused_must_use)]
        {
            write.shutdown();
        }
    }
}

pub(crate) fn reunite<T: AsyncWriteRent>(
    read: OwnedReadHalf<T>,
    write: OwnedWriteHalf<T>,
) -> Result<T, ReuniteError<T>> {
    if Rc::ptr_eq(&read.0, &write.0) {
        // we cannot execute drop for OwnedWriteHalf.
        unsafe {
            let _inner: Rc<UnsafeCell<T>> = std::mem::transmute(write);
        }
        // This unwrap cannot fail as the api does not allow creating more than two
        // Arcs, and we just dropped the other half.
        Ok(Rc::try_unwrap(read.0)
            .expect("try_unwrap failed in reunite")
            .into_inner())
    } else {
        Err(ReuniteError(read, write))
    }
}

/// Error indicating that two halves were not from the same socket, and thus
/// could not be reunited.
#[derive(Debug)]
pub struct ReuniteError<T: AsyncWriteRent>(pub OwnedReadHalf<T>, pub OwnedWriteHalf<T>);

impl<T> fmt::Display for ReuniteError<T>
where
    T: AsyncWriteRent,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tried to reunite halves")
    }
}

impl<T> Error for ReuniteError<T> where T: AsyncWriteRent + Debug {}
