use std::rc::Rc;

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    io::{AsyncReadRent, AsyncWriteRent},
};

use super::UnixStream;

/// ReadHalf.
#[derive(Debug)]
pub struct ReadHalf<'a>(&'a UnixStream);

/// WriteHalf.
#[derive(Debug)]
pub struct WriteHalf<'a>(&'a UnixStream);

pub(crate) fn split(stream: &mut UnixStream) -> (ReadHalf<'_>, WriteHalf<'_>) {
    (ReadHalf(&*stream), WriteHalf(&*stream))
}

impl<'t> AsyncReadRent for ReadHalf<'t> {
    type ReadFuture<'a, B>
    where
        't: 'a,
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ReadvFuture<'a, B>
    where
        't: 'a,
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;

    fn read<T: IoBufMut>(&self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        self.0.read(buf)
    }

    fn readv<T: IoVecBufMut>(&self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        self.0.readv(buf)
    }
}

impl<'t> AsyncWriteRent for WriteHalf<'t> {
    type WriteFuture<'a, B>
    where
        't: 'a,
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type WritevFuture<'a, B>
    where
        't: 'a,
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ShutdownFuture<'a>
    where
        't: 'a,
    = impl std::future::Future<Output = Result<(), std::io::Error>>;

    fn write<T: IoBuf>(&self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        self.0.write(buf)
    }

    fn writev<T: IoVecBuf>(&self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        self.0.writev(buf_vec)
    }

    fn shutdown(&self) -> Self::ShutdownFuture<'_> {
        self.0.shutdown()
    }
}

/// OwnedReadHalf.
#[derive(Debug)]
pub struct OwnedReadHalf(Rc<UnixStream>);

/// OwnedWriteHalf.
#[derive(Debug)]
pub struct OwnedWriteHalf(Rc<UnixStream>);

pub(crate) fn split_owned(stream: UnixStream) -> (OwnedReadHalf, OwnedWriteHalf) {
    let stream_shared = Rc::new(stream);
    (
        OwnedReadHalf(stream_shared.clone()),
        OwnedWriteHalf(stream_shared),
    )
}

impl AsyncReadRent for OwnedReadHalf {
    type ReadFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ReadvFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;

    fn read<T: IoBufMut>(&self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        self.0.read(buf)
    }

    fn readv<T: IoVecBufMut>(&self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        self.0.readv(buf)
    }
}

impl AsyncWriteRent for OwnedWriteHalf {
    type WriteFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type WritevFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ShutdownFuture<'a> = impl std::future::Future<Output = Result<(), std::io::Error>>;

    fn write<T: IoBuf>(&self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        self.0.write(buf)
    }

    fn writev<T: IoVecBuf>(&self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        self.0.writev(buf_vec)
    }

    fn shutdown(&self) -> Self::ShutdownFuture<'_> {
        self.0.shutdown()
    }
}
