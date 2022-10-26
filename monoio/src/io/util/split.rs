use std::{
    cell::UnsafeCell,
    error::Error,
    fmt::{self, Debug},
    future::Future,
    io,
    rc::Rc,
};

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    io::{AsyncReadRent, AsyncWriteRent},
};

/// Owned Read Half Part
#[derive(Debug)]
pub struct OwnedReadHalf<T>(pub Rc<UnsafeCell<T>>);
/// Owned Write Half Part
#[derive(Debug)]
pub struct OwnedWriteHalf<T>(pub Rc<UnsafeCell<T>>)
where
    T: AsyncWriteRent;
/// Borrowed Write Half Part
#[derive(Debug)]
pub struct WriteHalf<'cx, T>(pub &'cx T);
/// Borrowed Read Half Part
#[derive(Debug)]
pub struct ReadHalf<'cx, T>(pub &'cx T);
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

    /// Borrowed Read Split
    type Read<'cx>
    where
        Self: 'cx;
    /// Borrowed Write Split
    type Write<'cx>
    where
        Self: 'cx;

    /// Split into owned parts
    fn into_split(self) -> (Self::OwnedRead, Self::OwnedWrite);

    /// Split into borrowed parts
    fn split(&mut self) -> (Self::Read<'_>, Self::Write<'_>);
}

#[allow(clippy::cast_ref_to_mut)]
impl<'t, Inner> AsyncReadRent for ReadHalf<'t, Inner>
where
    Inner: AsyncReadRent,
{
    type ReadFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> + 'a where
        't: 'a, B: IoBufMut + 'a, Inner: 'a;
    type ReadvFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> + 'a where
        't: 'a, B: IoVecBufMut + 'a, Inner: 'a;

    #[inline]
    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T>
    where
        T: IoBufMut,
    {
        // Submit the read operation
        let raw_stream = unsafe { &mut *(self.0 as *const Inner as *mut Inner) };
        raw_stream.read(buf)
    }

    #[inline]
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        let raw_stream = unsafe { &mut *(self.0 as *const Inner as *mut Inner) };
        raw_stream.readv(buf)
    }
}

#[allow(clippy::cast_ref_to_mut)]
impl<'t, Inner> AsyncWriteRent for WriteHalf<'t, Inner>
where
    Inner: AsyncWriteRent,
{
    type WriteFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> + 'a where
        't: 'a, B: IoBuf + 'a, Inner: 'a;
    type WritevFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> + 'a where
        't: 'a, B: IoVecBuf + 'a, Inner: 'a;
    type FlushFuture<'a> = impl Future<Output = io::Result<()>> + 'a where
        't: 'a, Inner: 'a;
    type ShutdownFuture<'a> = impl Future<Output = io::Result<()>> + 'a where
        't: 'a, Inner: 'a;

    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T>
    where
        T: IoBuf,
    {
        // Submit the write operation
        let raw_stream = unsafe { &mut *(self.0 as *const Inner as *mut Inner) };
        raw_stream.write(buf)
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let raw_stream = unsafe { &mut *(self.0 as *const Inner as *mut Inner) };
        raw_stream.writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> Self::FlushFuture<'_> {
        let raw_stream = unsafe { &mut *(self.0 as *const Inner as *mut Inner) };
        raw_stream.flush()
    }

    #[inline]
    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        let raw_stream = unsafe { &mut *(self.0 as *const Inner as *mut Inner) };
        raw_stream.shutdown()
    }
}

impl<T> Splitable for T
where
    T: Split + AsyncReadRent + AsyncWriteRent,
{
    type Read<'cx> = ReadHalf<'cx, T> where Self: 'cx;

    type Write<'cx> = WriteHalf<'cx, T> where Self: 'cx;

    type OwnedRead = OwnedReadHalf<T>;

    type OwnedWrite = OwnedWriteHalf<T>;

    fn into_split(self) -> (Self::OwnedRead, Self::OwnedWrite) {
        let shared = Rc::new(UnsafeCell::new(self));
        (OwnedReadHalf(shared.clone()), OwnedWriteHalf(shared))
    }

    #[inline]
    fn split(&mut self) -> (Self::Read<'_>, Self::Write<'_>) {
        (ReadHalf(&*self), WriteHalf(&*self))
    }
}

impl<Inner> AsyncReadRent for OwnedReadHalf<Inner>
where
    Inner: AsyncReadRent,
{
    type ReadFuture<'a, T> = impl std::future::Future<Output = crate::BufResult<usize, T>> + 'a
    where
        Self: 'a,
        T: crate::buf::IoBufMut + 'a;

    type ReadvFuture<'a, T> = impl std::future::Future<Output = crate::BufResult<usize, T>> + 'a
    where
        Self: 'a,
        T: crate::buf::IoVecBufMut + 'a;

    #[inline]
    fn read<T: crate::buf::IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        let stream = unsafe { &mut *self.0.get() };
        stream.read(buf)
    }

    #[inline]
    fn readv<T: crate::buf::IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        let stream = unsafe { &mut *self.0.get() };
        stream.readv(buf)
    }
}

impl<Inner> AsyncWriteRent for OwnedWriteHalf<Inner>
where
    Inner: AsyncWriteRent,
{
    type WriteFuture<'a, T> =  impl Future<Output = crate::BufResult<usize, T>> + 'a
    where
        Self: 'a,
        T: crate::buf::IoBuf + 'a;

    type WritevFuture<'a, T> =  impl Future<Output = crate::BufResult<usize, T>> + 'a
    where
        Self: 'a,
        T: crate::buf::IoVecBuf + 'a;

    type FlushFuture<'a> = impl Future<Output = std::io::Result<()>> + 'a
    where
        Self: 'a;

    type ShutdownFuture<'a> = impl Future<Output = std::io::Result<()>> + 'a
    where
        Self: 'a;

    #[inline]
    fn write<T: crate::buf::IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        let stream = unsafe { &mut *self.0.get() };
        stream.write(buf)
    }

    #[inline]
    fn writev<T: crate::buf::IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let stream = unsafe { &mut *self.0.get() };
        stream.writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> Self::FlushFuture<'_> {
        let stream = unsafe { &mut *self.0.get() };
        stream.flush()
    }

    #[inline]
    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        let stream = unsafe { &mut *self.0.get() };
        stream.shutdown()
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
        write.shutdown();
    }
}

pub(crate) fn reunite<T: AsyncWriteRent>(
    read: OwnedReadHalf<T>,
    write: OwnedWriteHalf<T>,
) -> Result<T, ReuniteError<T>> {
    if Rc::ptr_eq(&read.0, &write.0) {
        drop(write);
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
