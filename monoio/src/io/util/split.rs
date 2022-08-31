use std::{cell::UnsafeCell, future::Future, rc::Rc};

use crate::io::{AsyncReadRent, AsyncWriteRent};

pub struct OwnedReadHalf<T>(Rc<UnsafeCell<T>>);
pub struct OwnedWriteHalf<T>(Rc<UnsafeCell<T>>);
pub struct WriteHalf<'cx, T>(&'cx T);
pub struct ReadHalf<'cx, T>(&'cx T);

/// This is a dummy unsafe trait to inform monoio,
/// the object with has this `Split` trait can be safely split
/// to read/write object in both form of `Owned` or `Borrowed`.
/// Note: monoio cannot guarantee whether the custom object can be
/// safely aplit to divided objects. Users should ensure the safety
/// by themselves.
pub unsafe trait Split {}

/// Inner split trait
pub trait _Split {
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
    fn split(&self) -> (Self::Read<'_>, Self::Write<'_>);
}

impl<T> _Split for T
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

    fn split(&self) -> (Self::Read<'_>, Self::Write<'_>) {
        (ReadHalf(&*self), WriteHalf(&*self))
    }
}

impl<Inner> AsyncReadRent for OwnedReadHalf<Inner>
where
    Inner: AsyncReadRent,
{
    type ReadFuture<'a, T> = impl std::future::Future<Output = crate::BufResult<usize, T>>
    where
        Self: 'a,
        T: crate::buf::IoBufMut + 'a;

    type ReadvFuture<'a, T> = impl std::future::Future<Output = crate::BufResult<usize, T>>
    where
        Self: 'a,
        T: crate::buf::IoVecBufMut + 'a;

    fn read<T: crate::buf::IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        let stream = unsafe { &mut *self.0.get() };
        stream.read(buf)
    }

    fn readv<T: crate::buf::IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        let stream = unsafe { &mut *self.0.get() };
        stream.readv(buf)
    }
}

impl<Inner> AsyncWriteRent for OwnedReadHalf<Inner>
where
    Inner: AsyncWriteRent,
{
    type WriteFuture<'a, T> =  impl Future<Output = crate::BufResult<usize, T>>
    where
        Self: 'a,
        T: crate::buf::IoBuf + 'a;

    type WritevFuture<'a, T> =  impl Future<Output = crate::BufResult<usize, T>>
    where
        Self: 'a,
        T: crate::buf::IoVecBuf + 'a;

    type FlushFuture<'a> = impl Future<Output = std::io::Result<()>>
    where
        Self: 'a;

    type ShutdownFuture<'a> = impl Future<Output = std::io::Result<()>>
    where
        Self: 'a;

    fn write<T: crate::buf::IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        let stream = unsafe { &mut *self.0.get() };
        stream.write(buf)
    }

    fn writev<T: crate::buf::IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let stream = unsafe { &mut *self.0.get() };
        stream.writev(buf_vec)
    }

    fn flush(&mut self) -> Self::FlushFuture<'_> {
        let stream = unsafe { &mut *self.0.get() };
        stream.flush()
    }

    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        let stream = unsafe { &mut *self.0.get() };
        stream.shutdown()
    }
}
