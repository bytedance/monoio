#[cfg(unix)]
use std::os::unix::prelude::AsRawFd;
use std::{cell::UnsafeCell, error::Error, fmt, future::Future, io, net::SocketAddr, rc::Rc};

use super::TcpStream;
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    io::{
        as_fd::{AsReadFd, AsWriteFd, SharedFdWrapper},
        AsyncReadRent, AsyncWriteRent,
    },
};

/// ReadHalf.
#[derive(Debug)]
pub struct ReadHalf<'a>(&'a TcpStream);

/// WriteHalf.
#[derive(Debug)]
pub struct WriteHalf<'a>(&'a TcpStream);

pub(crate) fn split(stream: &mut TcpStream) -> (ReadHalf<'_>, WriteHalf<'_>) {
    (ReadHalf(&*stream), WriteHalf(&*stream))
}

#[allow(clippy::cast_ref_to_mut)]
impl<'t> AsReadFd for ReadHalf<'t> {
    fn as_reader_fd(&mut self) -> &SharedFdWrapper {
        let raw_stream = unsafe { &mut *(self.0 as *const TcpStream as *mut TcpStream) };
        raw_stream.as_reader_fd()
    }
}

#[allow(clippy::cast_ref_to_mut)]
impl<'t> AsyncReadRent for ReadHalf<'t> {
    type ReadFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        't: 'a, B: IoBufMut + 'a;
    type ReadvFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        't: 'a, B: IoVecBufMut + 'a,;

    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        let raw_stream = unsafe { &mut *(self.0 as *const TcpStream as *mut TcpStream) };
        raw_stream.read(buf)
    }

    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        let raw_stream = unsafe { &mut *(self.0 as *const TcpStream as *mut TcpStream) };
        raw_stream.readv(buf)
    }
}

#[allow(clippy::cast_ref_to_mut)]
impl<'t> AsWriteFd for WriteHalf<'t> {
    fn as_writer_fd(&mut self) -> &SharedFdWrapper {
        let raw_stream = unsafe { &mut *(self.0 as *const TcpStream as *mut TcpStream) };
        raw_stream.as_writer_fd()
    }
}

#[allow(clippy::cast_ref_to_mut)]
impl<'t> AsyncWriteRent for WriteHalf<'t> {
    type WriteFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> where
        't: 'a, B: IoBuf + 'a;
    type WritevFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> where
        't: 'a, B: IoVecBuf + 'a;
    type FlushFuture<'a> = impl Future<Output = io::Result<()>> where
        't: 'a;
    type ShutdownFuture<'a> = impl Future<Output = io::Result<()>> where
        't: 'a;

    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        let raw_stream = unsafe { &mut *(self.0 as *const TcpStream as *mut TcpStream) };
        raw_stream.write(buf)
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let raw_stream = unsafe { &mut *(self.0 as *const TcpStream as *mut TcpStream) };
        raw_stream.writev(buf_vec)
    }

    fn flush(&mut self) -> Self::FlushFuture<'_> {
        // Tcp stream does not need flush.
        async move { Ok(()) }
    }

    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        let raw_stream = unsafe { &mut *(self.0 as *const TcpStream as *mut TcpStream) };
        raw_stream.shutdown()
    }
}

/// OwnedReadHalf.
#[derive(Debug)]
pub struct OwnedReadHalf(Rc<UnsafeCell<TcpStream>>);

/// OwnedWriteHalf.
#[derive(Debug)]
pub struct OwnedWriteHalf(Rc<UnsafeCell<TcpStream>>);

pub(crate) fn split_owned(stream: TcpStream) -> (OwnedReadHalf, OwnedWriteHalf) {
    let stream_shared = Rc::new(UnsafeCell::new(stream));
    (
        OwnedReadHalf(stream_shared.clone()),
        OwnedWriteHalf(stream_shared),
    )
}

pub(crate) fn reunite(
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
) -> Result<TcpStream, ReuniteError> {
    if Rc::ptr_eq(&read.0, &write.0) {
        drop(write);
        // This unwrap cannot fail as the api does not allow creating more than two
        // Arcs, and we just dropped the other half.
        Ok(Rc::try_unwrap(read.0)
            .expect("TcpStream: try_unwrap failed in reunite")
            .into_inner())
    } else {
        Err(ReuniteError(read, write))
    }
}

/// Error indicating that two halves were not from the same socket, and thus
/// could not be reunited.
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
    pub fn reunite(self, other: OwnedWriteHalf) -> Result<TcpStream, ReuniteError> {
        reunite(self, other)
    }

    /// Returns the remote address that this stream is connected to.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        unsafe { &*self.0.get() }.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        unsafe { &*self.0.get() }.local_addr()
    }
}

impl AsReadFd for OwnedReadHalf {
    fn as_reader_fd(&mut self) -> &SharedFdWrapper {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.as_reader_fd()
    }
}

impl AsyncReadRent for OwnedReadHalf {
    type ReadFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: IoBufMut + 'a;
    type ReadvFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: IoVecBufMut + 'a;

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

impl OwnedWriteHalf {
    /// Attempts to put the two halves of a `TcpStream` back together and
    /// recover the original socket. Succeeds only if the two halves
    /// originated from the same call to [`into_split`].
    ///
    /// [`into_split`]: TcpStream::into_split()
    pub fn reunite(self, other: OwnedReadHalf) -> Result<TcpStream, ReuniteError> {
        reunite(other, self)
    }

    /// Returns the remote address that this stream is connected to.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        unsafe { &*self.0.get() }.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        unsafe { &*self.0.get() }.local_addr()
    }
}

impl AsWriteFd for OwnedWriteHalf {
    fn as_writer_fd(&mut self) -> &SharedFdWrapper {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.as_writer_fd()
    }
}

impl AsyncWriteRent for OwnedWriteHalf {
    type WriteFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> where
        B: IoBuf + 'a;
    type WritevFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> where
        B: IoVecBuf + 'a;
    type FlushFuture<'a> = impl Future<Output = io::Result<()>>;
    type ShutdownFuture<'a> = impl Future<Output = io::Result<()>>;

    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.write(buf)
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.writev(buf_vec)
    }

    fn flush(&mut self) -> Self::FlushFuture<'_> {
        // Tcp stream does not need flush.
        async move { Ok(()) }
    }

    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.shutdown()
    }
}

#[cfg(unix)]
impl Drop for OwnedWriteHalf {
    fn drop(&mut self) {
        let raw_stream = unsafe { &mut *self.0.get() };
        let fd = raw_stream.as_raw_fd();
        unsafe { libc::shutdown(fd, libc::SHUT_WR) };
    }
}

#[cfg(windows)]
impl Drop for OwnedWriteHalf {
    fn drop(&mut self) {
        unimplemented!()
    }
}
