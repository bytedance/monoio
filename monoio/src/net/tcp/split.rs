use std::{io, net::SocketAddr};

use super::TcpStream;
use crate::io::{
    as_fd::{AsReadFd, AsWriteFd, SharedFdWrapper},
    OwnedReadHalf, OwnedWriteHalf,
};

/// OwnedReadHalf.
pub type TcpOwnedReadHalf = OwnedReadHalf<TcpStream>;
/// OwnedWriteHalf
pub type TcpOwnedWriteHalf = OwnedWriteHalf<TcpStream>;

impl TcpOwnedReadHalf {
    /// Returns the remote address that this stream is connected to.
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        unsafe { &*self.0.get() }.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        unsafe { &*self.0.get() }.local_addr()
    }
}

impl AsReadFd for TcpOwnedReadHalf {
    #[inline]
    fn as_reader_fd(&mut self) -> &SharedFdWrapper {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.as_reader_fd()
    }
}

impl TcpOwnedWriteHalf {
    /// Returns the remote address that this stream is connected to.
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        unsafe { &*self.0.get() }.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        unsafe { &*self.0.get() }.local_addr()
    }
}

impl AsWriteFd for TcpOwnedWriteHalf {
    #[inline]
    fn as_writer_fd(&mut self) -> &SharedFdWrapper {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.as_writer_fd()
    }
}
