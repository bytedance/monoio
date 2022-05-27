use super::split::{split, split_owned, OwnedReadHalf, OwnedWriteHalf, ReadHalf, WriteHalf};
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    driver::{op::Op, shared_fd::SharedFd},
    io::{AsyncReadRent, AsyncWriteRent},
};

use std::{
    cell::UnsafeCell,
    io,
    net::{SocketAddr, ToSocketAddrs},
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
    time::Duration,
};

/// TcpStream
pub struct TcpStream {
    fd: SharedFd,
    meta: StreamMeta,
}

impl TcpStream {
    pub(crate) fn from_shared_fd(fd: SharedFd) -> Self {
        let meta = StreamMeta::new(fd.raw_fd());
        #[cfg(feature = "zero-copy")]
        // enable SOCK_ZEROCOPY
        meta.set_zero_copy();

        Self { fd, meta }
    }

    /// Open a TCP connection to a remote host.
    /// Note: This function may block the current thread while resolution is
    /// performed.
    // TODO(chihai): Fix it, maybe spawn_blocking like tokio.
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        // TODO(chihai): loop for all addrs
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "empty address"))?;

        Self::connect_addr(addr).await
    }

    /// Establishe a connection to the specified `addr`.
    pub async fn connect_addr(addr: SocketAddr) -> io::Result<Self> {
        let op = Op::connect(libc::SOCK_STREAM, addr)?;
        let completion = op.await;
        completion.meta.result?;

        let mut stream = TcpStream::from_shared_fd(completion.data.fd);
        // wait write ready
        // TODO: not use write to detect writable
        let _ = stream.write([]).await;
        // getsockopt
        let sys_socket = unsafe { std::net::TcpStream::from_raw_fd(stream.fd.raw_fd()) };
        let err = sys_socket.take_error();
        let _ = sys_socket.into_raw_fd();
        if let Some(e) = err? {
            return Err(e);
        }
        Ok(stream)
    }

    /// Return the local address that this stream is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.meta.local_addr()
    }

    /// Return the remote address that this stream is connected to.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.meta.peer_addr()
    }

    /// Get the value of the `TCP_NODELAY` option on this socket.
    pub fn nodelay(&self) -> io::Result<bool> {
        self.meta.no_delay()
    }

    /// Set the value of the `TCP_NODELAY` option on this socket.
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.meta.set_no_delay(nodelay)
    }

    /// Set the value of the `SO_KEEPALIVE` option on this socket.
    pub fn set_tcp_keepalive(
        &self,
        time: Option<Duration>,
        interval: Option<Duration>,
        retries: Option<u32>,
    ) -> io::Result<()> {
        self.meta.set_tcp_keepalive(time, interval, retries)
    }

    /// Split stream into read and write halves.
    #[allow(clippy::needless_lifetimes)]
    pub fn split<'a>(&'a mut self) -> (ReadHalf<'a>, WriteHalf<'a>) {
        split(self)
    }

    /// Split stream into read and write halves with ownership.
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        split_owned(self)
    }
}

impl IntoRawFd for TcpStream {
    fn into_raw_fd(self) -> RawFd {
        self.fd
            .try_unwrap()
            .expect("unexpected multiple reference to rawfd")
    }
}

impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl std::fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpStream").field("fd", &self.fd).finish()
    }
}

impl AsyncWriteRent for TcpStream {
    type WriteFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
    type WritevFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
    type ShutdownFuture<'a> = impl std::future::Future<Output = Result<(), std::io::Error>>;

    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        let op = Op::send(&self.fd, buf).unwrap();
        op.write()
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let op = Op::writev(&self.fd, buf_vec).unwrap();
        op.write()
    }

    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        // We could use shutdown op here, which requires kernel 5.11+.
        // However, for simplicity, we just close the socket using direct syscall.
        let fd = self.as_raw_fd();
        async move {
            match unsafe { libc::shutdown(fd, libc::SHUT_WR) } {
                -1 => Err(io::Error::last_os_error()),
                _ => Ok(()),
            }
        }
    }
}

impl AsyncReadRent for TcpStream {
    type ReadFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
    type ReadvFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;

    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        let op = Op::recv(&self.fd, buf).unwrap();
        op.read()
    }

    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        let op = Op::readv(&self.fd, buf).unwrap();
        op.read()
    }
}

struct StreamMeta {
    socket: Option<socket2::Socket>,
    meta: UnsafeCell<Meta>,
}

#[derive(Debug, Default, Clone)]
struct Meta {
    local_addr: Option<SocketAddr>,
    peer_addr: Option<SocketAddr>,
}

impl StreamMeta {
    fn new(fd: RawFd) -> Self {
        Self {
            socket: unsafe { Some(socket2::Socket::from_raw_fd(fd)) },
            meta: Default::default(),
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        let meta = unsafe { &mut *self.meta.get() };
        if let Some(addr) = meta.local_addr {
            return Ok(addr);
        }

        let ret = self
            .socket
            .as_ref()
            .unwrap()
            .local_addr()
            .map(|addr| addr.as_socket().expect("tcp socket is expected"));
        if let Ok(addr) = ret {
            meta.local_addr = Some(addr);
        }
        ret
    }

    fn peer_addr(&self) -> io::Result<SocketAddr> {
        let meta = unsafe { &mut *self.meta.get() };
        if let Some(addr) = meta.peer_addr {
            return Ok(addr);
        }

        let ret = self
            .socket
            .as_ref()
            .unwrap()
            .peer_addr()
            .map(|addr| addr.as_socket().expect("tcp socket is expected"));
        if let Ok(addr) = ret {
            meta.peer_addr = Some(addr);
        }
        ret
    }

    fn no_delay(&self) -> io::Result<bool> {
        self.socket.as_ref().unwrap().nodelay()
    }

    fn set_no_delay(&self, no_delay: bool) -> io::Result<()> {
        self.socket.as_ref().unwrap().set_nodelay(no_delay)
    }

    fn set_tcp_keepalive(
        &self,
        time: Option<Duration>,
        interval: Option<Duration>,
        retries: Option<u32>,
    ) -> io::Result<()> {
        let mut t = socket2::TcpKeepalive::new();
        if let Some(time) = time {
            t = t.with_time(time)
        }
        if let Some(interval) = interval {
            t = t.with_interval(interval)
        }
        if let Some(retries) = retries {
            t = t.with_retries(retries)
        }
        self.socket.as_ref().unwrap().set_tcp_keepalive(&t)
    }

    #[cfg(feature = "zero-copy")]
    fn set_zero_copy(&self) {
        #[cfg(target_os = "linux")]
        unsafe {
            let fd = self.socket.as_ref().unwrap().as_raw_fd();
            let v: libc::c_int = 1;
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_ZEROCOPY,
                &v as *const _ as *const _,
                std::mem::size_of::<libc::c_int>() as _,
            );
        }
    }
}

impl Drop for StreamMeta {
    fn drop(&mut self) {
        self.socket.take().unwrap().into_raw_fd();
    }
}
