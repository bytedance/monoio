//! This module provide a poll-io style interface for UnixStream.

use std::{io, os::fd::AsRawFd};

use super::{SocketAddr, UnixStream};
use crate::driver::op::Op;

/// A UnixStream with poll-io style interface.
/// Using this struct, you can use UnixStream in a poll-like way.
/// Underlying, it is based on a uring-based epoll.
#[derive(Debug)]
pub struct UnixStreamPoll(UnixStream);

impl crate::io::IntoPollIo for UnixStream {
    type PollIo = UnixStreamPoll;

    #[inline]
    fn try_into_poll_io(self) -> Result<Self::PollIo, (std::io::Error, Self)> {
        self.try_into_poll_io()
    }
}

impl UnixStream {
    /// Convert to poll-io style UnixStreamPoll
    #[inline]
    pub fn try_into_poll_io(mut self) -> Result<UnixStreamPoll, (io::Error, UnixStream)> {
        match self.fd.cvt_poll() {
            Ok(_) => Ok(UnixStreamPoll(self)),
            Err(e) => Err((e, self)),
        }
    }
}

impl crate::io::IntoCompIo for UnixStreamPoll {
    type CompIo = UnixStream;

    #[inline]
    fn try_into_comp_io(self) -> Result<Self::CompIo, (std::io::Error, Self)> {
        self.try_into_comp_io()
    }
}

impl UnixStreamPoll {
    /// Convert to normal UnixStream
    #[inline]
    pub fn try_into_comp_io(mut self) -> Result<UnixStream, (io::Error, UnixStreamPoll)> {
        match self.0.fd.cvt_comp() {
            Ok(_) => Ok(self.0),
            Err(e) => Err((e, self)),
        }
    }
}

impl tokio::io::AsyncRead for UnixStreamPoll {
    #[inline]
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        unsafe {
            let slice = buf.unfilled_mut();
            let raw_buf = crate::buf::RawBuf::new(slice.as_ptr() as *const u8, slice.len());
            let mut recv = Op::recv_raw(&self.0.fd, raw_buf);
            let ret = ready!(crate::driver::op::PollLegacy::poll_io(&mut recv, cx));

            std::task::Poll::Ready(ret.result.map(|n| {
                let n = n.into_inner();
                buf.assume_init(n as usize);
                buf.advance(n as usize);
            }))
        }
    }
}

impl tokio::io::AsyncWrite for UnixStreamPoll {
    #[inline]
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        unsafe {
            let raw_buf = crate::buf::RawBuf::new(buf.as_ptr(), buf.len());
            let mut send = Op::send_raw(&self.0.fd, raw_buf);
            let ret = ready!(crate::driver::op::PollLegacy::poll_io(&mut send, cx));

            std::task::Poll::Ready(ret.result.map(|n| n.into_inner() as usize))
        }
    }

    #[inline]
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        let fd = self.0.as_raw_fd();
        let res = match unsafe { libc::shutdown(fd, libc::SHUT_WR) } {
            -1 => Err(io::Error::last_os_error()),
            _ => Ok(()),
        };
        std::task::Poll::Ready(res)
    }

    #[inline]
    fn poll_write_vectored(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        unsafe {
            let raw_buf =
                crate::buf::RawBufVectored::new(bufs.as_ptr() as *const libc::iovec, bufs.len());
            let mut writev = Op::writev_raw(&self.0.fd, raw_buf);
            let ret = ready!(crate::driver::op::PollLegacy::poll_io(&mut writev, cx));

            std::task::Poll::Ready(ret.result.map(|n| n.into_inner() as usize))
        }
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }
}

impl UnixStreamPoll {
    /// Returns the socket address of the local half of this connection.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }

    /// Returns the socket address of the remote half of this connection.
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }
}
