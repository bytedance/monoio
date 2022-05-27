use std::{cell::UnsafeCell, error::Error, fmt, io, rc::Rc};

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    io::{AsyncReadRent, AsyncWriteRent},
};

use super::{SocketAddr, UnixStream};

/// ReadHalf.
#[derive(Debug)]
pub struct ReadHalf<'a>(&'a UnixStream);

/// WriteHalf.
#[derive(Debug)]
pub struct WriteHalf<'a>(&'a UnixStream);

pub(crate) fn split(stream: &mut UnixStream) -> (ReadHalf<'_>, WriteHalf<'_>) {
    (ReadHalf(&*stream), WriteHalf(&*stream))
}

#[allow(clippy::cast_ref_to_mut)]
impl<'t> AsyncReadRent for ReadHalf<'t> {
    type ReadFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        't: 'a, B: 'a,;
    type ReadvFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        't: 'a, B: 'a,;

    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        let raw_stream = unsafe { &mut *(self.0 as *const UnixStream as *mut UnixStream) };
        raw_stream.read(buf)
    }

    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        let raw_stream = unsafe { &mut *(self.0 as *const UnixStream as *mut UnixStream) };
        raw_stream.readv(buf)
    }
}

#[allow(clippy::cast_ref_to_mut)]
impl<'t> AsyncWriteRent for WriteHalf<'t> {
    type WriteFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        't: 'a, B: 'a;
    type WritevFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        't: 'a, B: 'a,;
    type ShutdownFuture<'a> = impl std::future::Future<Output = Result<(), std::io::Error>> where
        't: 'a,;

    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        let raw_stream = unsafe { &mut *(self.0 as *const UnixStream as *mut UnixStream) };
        raw_stream.write(buf)
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let raw_stream = unsafe { &mut *(self.0 as *const UnixStream as *mut UnixStream) };
        raw_stream.writev(buf_vec)
    }

    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        let raw_stream = unsafe { &mut *(self.0 as *const UnixStream as *mut UnixStream) };
        raw_stream.shutdown()
    }
}

/// OwnedReadHalf.
#[derive(Debug)]
pub struct OwnedReadHalf(Rc<UnsafeCell<UnixStream>>);

/// OwnedWriteHalf.
#[derive(Debug)]
pub struct OwnedWriteHalf(Rc<UnsafeCell<UnixStream>>);

pub(crate) fn split_owned(stream: UnixStream) -> (OwnedReadHalf, OwnedWriteHalf) {
    let stream_shared = Rc::new(UnsafeCell::new(stream));
    (
        OwnedReadHalf(stream_shared.clone()),
        OwnedWriteHalf(stream_shared),
    )
}

pub(crate) fn reunite(
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
) -> Result<UnixStream, ReuniteError> {
    if Rc::ptr_eq(&read.0, &write.0) {
        drop(write);
        // This unwrap cannot fail as the api does not allow creating more than two Arcs,
        // and we just dropped the other half.
        Ok(Rc::try_unwrap(read.0)
            .expect("UnixStream: try_unwrap failed in reunite")
            .into_inner())
    } else {
        Err(ReuniteError(read, write))
    }
}

/// Error indicating that two halves were not from the same socket, and thus could
/// not be reunited.
#[derive(Debug)]
pub struct ReuniteError(pub OwnedReadHalf, pub OwnedWriteHalf);

impl fmt::Display for ReuniteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tried to reunite halves that are not from the same socket"
        )
    }
}

impl Error for ReuniteError {}

impl OwnedReadHalf {
    /// Attempts to put the two halves of a `TcpStream` back together and
    /// recover the original socket. Succeeds only if the two halves
    /// originated from the same call to [`into_split`].
    ///
    /// [`into_split`]: TcpStream::into_split()
    pub fn reunite(self, other: OwnedWriteHalf) -> Result<UnixStream, ReuniteError> {
        reunite(self, other)
    }

    /// Returns the remote address that this stream is connected to.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.local_addr()
    }
}

impl AsyncReadRent for OwnedReadHalf {
    type ReadFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
    type ReadvFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;

    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.read(buf)
    }

    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.readv(buf)
    }
}

impl AsyncWriteRent for OwnedWriteHalf {
    type WriteFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
    type WritevFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
    type ShutdownFuture<'a> = impl std::future::Future<Output = Result<(), std::io::Error>>;

    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.write(buf)
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.writev(buf_vec)
    }

    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.shutdown()
    }
}
