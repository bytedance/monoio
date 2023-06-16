use std::io;

use super::{SocketAddr, UnixStream};
use crate::io::{
    as_fd::{AsReadFd, AsWriteFd, SharedFdWrapper},
    OwnedReadHalf, OwnedWriteHalf,
};

/// OwnedReadHalf.
pub type UnixOwnedReadHalf = OwnedReadHalf<UnixStream>;

/// OwnedWriteHalf.
pub type UnixOwnedWriteHalf = OwnedWriteHalf<UnixStream>;

impl UnixOwnedReadHalf {
    /// Returns the remote address that this stream is connected to.
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.peer_addr()
    }

    /// Returns the local address that this stream is bound to.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.local_addr()
    }
}

impl AsReadFd for UnixOwnedReadHalf {
    #[inline]
    fn as_reader_fd(&mut self) -> &SharedFdWrapper {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.as_reader_fd()
    }
}

impl AsWriteFd for UnixOwnedWriteHalf {
    #[inline]
    fn as_writer_fd(&mut self) -> &SharedFdWrapper {
        let raw_stream = unsafe { &mut *self.0.get() };
        raw_stream.as_writer_fd()
    }
}
