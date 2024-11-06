//! This module provide a poll-io style interface for TcpStream.

use std::{io, net::SocketAddr, time::Duration};

#[cfg(unix)]
use {
    libc::{shutdown, SHUT_WR},
    std::os::fd::AsRawFd,
};
#[cfg(windows)]
use {
    std::os::windows::io::AsRawSocket,
    windows_sys::Win32::Networking::WinSock::{shutdown, SD_SEND as SHUT_WR},
};

use super::TcpStream;
use crate::driver::op::Op;

/// A TcpStream with poll-io style interface.
/// Using this struct, you can use TcpStream in a poll-like way.
/// Underlying, it is based on a uring-based epoll.
#[derive(Debug)]
pub struct TcpStreamPoll(TcpStream);

impl crate::io::IntoPollIo for TcpStream {
    type PollIo = TcpStreamPoll;

    #[inline]
    fn try_into_poll_io(self) -> Result<Self::PollIo, (std::io::Error, Self)> {
        self.try_into_poll_io()
    }
}

impl TcpStream {
    /// Convert to poll-io style TcpStreamPoll
    #[inline]
    pub fn try_into_poll_io(mut self) -> Result<TcpStreamPoll, (io::Error, TcpStream)> {
        match self.fd.cvt_poll() {
            Ok(_) => Ok(TcpStreamPoll(self)),
            Err(e) => Err((e, self)),
        }
    }
}

impl crate::io::IntoCompIo for TcpStreamPoll {
    type CompIo = TcpStream;

    #[inline]
    fn try_into_comp_io(self) -> Result<Self::CompIo, (std::io::Error, Self)> {
        self.try_into_comp_io()
    }
}

impl TcpStreamPoll {
    /// Convert to normal TcpStream
    #[inline]
    pub fn try_into_comp_io(mut self) -> Result<TcpStream, (io::Error, TcpStreamPoll)> {
        match self.0.fd.cvt_comp() {
            Ok(_) => Ok(self.0),
            Err(e) => Err((e, self)),
        }
    }
}

impl tokio::io::AsyncRead for TcpStreamPoll {
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

impl tokio::io::AsyncWrite for TcpStreamPoll {
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
        #[cfg(unix)]
        let fd = self.0.as_raw_fd();
        #[cfg(windows)]
        let fd = self.0.as_raw_socket() as _;
        let res = match unsafe { shutdown(fd, SHUT_WR) } {
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
            let raw_buf = crate::buf::RawBufVectored::new(bufs.as_ptr() as _, bufs.len());
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

impl TcpStreamPoll {
    /// Return the local address that this stream is bound to.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }

    /// Return the remote address that this stream is connected to.
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }

    /// Get the value of the `TCP_NODELAY` option on this socket.
    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.0.nodelay()
    }

    /// Set the value of the `TCP_NODELAY` option on this socket.
    #[inline]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.0.set_nodelay(nodelay)
    }

    /// Set the value of the `SO_KEEPALIVE` option on this socket.
    #[inline]
    pub fn set_tcp_keepalive(
        &self,
        time: Option<Duration>,
        interval: Option<Duration>,
        retries: Option<u32>,
    ) -> io::Result<()> {
        self.0.set_tcp_keepalive(time, interval, retries)
    }
}

#[cfg(unix)]
impl AsRawFd for TcpStreamPoll {
    #[inline]
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(windows)]
impl AsRawSocket for TcpStreamPoll {
    fn as_raw_socket(&self) -> std::os::windows::io::RawSocket {
        self.0.as_raw_socket()
    }
}
