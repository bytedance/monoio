#[cfg(unix)]
use std::os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::prelude::{AsRawHandle, IntoRawHandle, RawHandle};
use std::{
    cell::UnsafeCell,
    future::Future,
    io,
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    driver::{op::Op, shared_fd::SharedFd},
    io::{
        as_fd::{AsReadFd, AsWriteFd, SharedFdWrapper},
        AsyncReadRent, AsyncWriteRent, Split,
    },
};

const EMPTY_SLICE: [u8; 0] = [];

/// TcpStream
pub struct TcpStream {
    fd: SharedFd,
    meta: StreamMeta,
}

/// TcpStream is safe to split to two parts
unsafe impl Split for TcpStream {}

impl TcpStream {
    pub(crate) fn from_shared_fd(fd: SharedFd) -> Self {
        #[cfg(unix)]
        let meta = StreamMeta::new(fd.raw_fd());
        #[cfg(windows)]
        let meta = StreamMeta::new(fd.raw_handle());
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

    #[cfg(unix)]
    /// Establishe a connection to the specified `addr`.
    pub async fn connect_addr(addr: SocketAddr) -> io::Result<Self> {
        let op = Op::connect(libc::SOCK_STREAM, addr)?;
        let completion = op.await;
        completion.meta.result?;

        let mut stream = TcpStream::from_shared_fd(completion.data.fd);
        // wait write ready
        // TODO: not use write to detect writable
        let _ = stream.write(&EMPTY_SLICE).await;
        // getsockopt
        let sys_socket = unsafe { std::net::TcpStream::from_raw_fd(stream.fd.raw_fd()) };
        let err = sys_socket.take_error();
        let _ = sys_socket.into_raw_fd();
        if let Some(e) = err? {
            return Err(e);
        }
        Ok(stream)
    }

    #[cfg(windows)]
    /// Establishe a connection to the specified `addr`.
    pub async fn connect_addr(addr: SocketAddr) -> io::Result<Self> {
        unimplemented!()
    }

    /// Return the local address that this stream is bound to.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.meta.local_addr()
    }

    /// Return the remote address that this stream is connected to.
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.meta.peer_addr()
    }

    /// Get the value of the `TCP_NODELAY` option on this socket.
    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.meta.no_delay()
    }

    /// Set the value of the `TCP_NODELAY` option on this socket.
    #[inline]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.meta.set_no_delay(nodelay)
    }

    /// Set the value of the `SO_KEEPALIVE` option on this socket.
    #[inline]
    pub fn set_tcp_keepalive(
        &self,
        time: Option<Duration>,
        interval: Option<Duration>,
        retries: Option<u32>,
    ) -> io::Result<()> {
        self.meta.set_tcp_keepalive(time, interval, retries)
    }

    /// Creates new `TcpStream` from a `std::net::TcpStream`.
    pub fn from_std(stream: std::net::TcpStream) -> io::Result<Self> {
        let fd = stream.into_raw_fd();
        Ok(Self::from_shared_fd(SharedFd::new(fd)?))
    }
}

impl AsReadFd for TcpStream {
    #[inline]
    fn as_reader_fd(&mut self) -> &SharedFdWrapper {
        SharedFdWrapper::new(&self.fd)
    }
}

impl AsWriteFd for TcpStream {
    #[inline]
    fn as_writer_fd(&mut self) -> &SharedFdWrapper {
        SharedFdWrapper::new(&self.fd)
    }
}

#[cfg(unix)]
impl IntoRawFd for TcpStream {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.fd
            .try_unwrap()
            .expect("unexpected multiple reference to rawfd")
    }
}
#[cfg(unix)]
impl AsRawFd for TcpStream {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

#[cfg(windows)]
impl IntoRawHandle for TcpStream {
    #[inline]
    fn into_raw_handle(self) -> RawHandle {
        self.fd
            .try_unwrap()
            .expect("unexpected multiple reference to rawfd")
    }
}
#[cfg(windows)]
impl AsRawHandle for TcpStream {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.fd.raw_handle()
    }
}

impl std::fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpStream").field("fd", &self.fd).finish()
    }
}

impl AsyncWriteRent for TcpStream {
    type WriteFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> where
        B: IoBuf + 'a;
    type WritevFuture<'a, B> = impl Future<Output = crate::BufResult<usize, B>> where
        B: IoVecBuf + 'a;
    type FlushFuture<'a> = impl Future<Output = io::Result<()>>;
    type ShutdownFuture<'a> = impl Future<Output = io::Result<()>>;

    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        let op = Op::send(&self.fd, buf).unwrap();
        op.write()
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let op = Op::writev(&self.fd, buf_vec).unwrap();
        op.write()
    }

    #[inline]
    fn flush(&mut self) -> Self::FlushFuture<'_> {
        // Tcp stream does not need flush.
        async move { Ok(()) }
    }

    #[cfg(unix)]
    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        // We could use shutdown op here, which requires kernel 5.11+.
        // However, for simplicity, we just close the socket using direct syscall.
        let fd = self.as_raw_fd();
        let res = match unsafe { libc::shutdown(fd, libc::SHUT_WR) } {
            -1 => Err(io::Error::last_os_error()),
            _ => Ok(()),
        };
        async move { res }
    }

    #[cfg(windows)]
    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        async { unimplemented!() }
    }
}

impl AsyncReadRent for TcpStream {
    type ReadFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: IoBufMut + 'a;
    type ReadvFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: IoVecBufMut + 'a;

    #[inline]
    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        let op = Op::recv(&self.fd, buf).unwrap();
        op.read()
    }

    #[inline]
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        let op = Op::readv(&self.fd, buf).unwrap();
        op.read()
    }
}

#[cfg(all(unix, feature = "legacy", feature = "tokio-compat"))]
impl tokio::io::AsyncRead for TcpStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        unsafe {
            let slice = buf.unfilled_mut();
            let raw_buf = crate::buf::RawBuf::new(slice.as_ptr() as *const u8, slice.len());
            let mut recv = Op::recv_raw(&self.fd, raw_buf);
            let ret = ready!(crate::driver::op::PollLegacy::poll_legacy(&mut recv, cx));

            std::task::Poll::Ready(ret.result.map(|n| {
                buf.assume_init(n as usize);
                buf.advance(n as usize);
            }))
        }
    }
}

#[cfg(all(unix, feature = "legacy", feature = "tokio-compat"))]
impl tokio::io::AsyncWrite for TcpStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        unsafe {
            let raw_buf = crate::buf::RawBuf::new(buf.as_ptr() as *const u8, buf.len());
            let mut send = Op::send_raw(&self.fd, raw_buf);
            let ret = ready!(crate::driver::op::PollLegacy::poll_legacy(&mut send, cx));

            std::task::Poll::Ready(ret.result.map(|n| n as usize))
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        let fd = self.as_raw_fd();
        let res = match unsafe { libc::shutdown(fd, libc::SHUT_WR) } {
            -1 => Err(io::Error::last_os_error()),
            _ => Ok(()),
        };
        std::task::Poll::Ready(res)
    }

    fn poll_write_vectored(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        unsafe {
            let raw_buf =
                crate::buf::RawBufVectored::new(bufs.as_ptr() as *const libc::iovec, bufs.len());
            let mut writev = Op::writev_raw(&self.fd, raw_buf);
            let ret = ready!(crate::driver::op::PollLegacy::poll_legacy(&mut writev, cx));

            std::task::Poll::Ready(ret.result.map(|n| n as usize))
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
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
    #[cfg(unix)]
    fn new(fd: RawFd) -> Self {
        Self {
            socket: unsafe { Some(socket2::Socket::from_raw_fd(fd)) },
            meta: Default::default(),
        }
    }
    #[cfg(windows)]
    fn new(fd: RawHandle) -> Self {
        unimplemented!()
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
        #[cfg(unix)]
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
        #[cfg(unix)]
        self.socket.take().unwrap().into_raw_fd();
        #[cfg(windows)]
        unimplemented!()
    }
}
