use std::{cell::UnsafeCell, io};

use monoio::{
    io::{AsyncReadRent, AsyncWriteRent},
    net::TcpStream,
    BufResult,
};

use crate::{box_future::MaybeArmedBoxFuture, buf::RawBuf};

#[derive(Default)]
struct Dst(Option<(*const u8, usize)>);

impl Dst {
    fn check_and_to_rawbuf(&mut self, ptr: *const u8, len: usize) -> RawBuf {
        // Set or check read_dst
        // Note: the check can not prevent memory crash when user misuse it.
        match self.0 {
            None => {
                self.0 = Some((ptr, len));
            }
            Some((last_ptr, last_len)) => {
                assert_eq!(last_ptr, ptr);
                assert_eq!(last_len, len);
            }
        }
        RawBuf::new(ptr, len)
    }

    fn reset(&mut self) {
        self.0 = None;
    }
}

pub struct TcpStreamCompat {
    stream: UnsafeCell<TcpStream>,
    read_dst: Dst,
    write_dst: Dst,

    read_fut: MaybeArmedBoxFuture<BufResult<usize, RawBuf>>,
    write_fut: MaybeArmedBoxFuture<BufResult<usize, RawBuf>>,
    flush_fut: MaybeArmedBoxFuture<io::Result<()>>,
    shutdown_fut: MaybeArmedBoxFuture<io::Result<()>>,
}

impl From<TcpStreamCompat> for TcpStream {
    fn from(stream: TcpStreamCompat) -> Self {
        stream.stream.into_inner()
    }
}

impl TcpStreamCompat {
    /// Creates a new `TcpStreamCompat` from a monoio `TcpStream`.
    ///
    /// # Safety
    /// User must ensure that the data slice pointer and length is always
    /// valid and the same among different calls before Poll::Ready returns.
    pub unsafe fn new(stream: TcpStream) -> Self {
        Self {
            stream: UnsafeCell::new(stream),
            read_dst: Default::default(),
            write_dst: Default::default(),
            read_fut: Default::default(),
            write_fut: Default::default(),
            flush_fut: Default::default(),
            shutdown_fut: Default::default(),
        }
    }
}

impl tokio::io::AsyncRead for TcpStreamCompat {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let buf_unfilled = unsafe { buf.unfilled_mut() };
        let (ptr, len) = (buf_unfilled.as_ptr() as *const u8, buf_unfilled.len());

        // Set or check read_dst
        // Note: the check can not prevent memory crash when user misuse it.
        let raw_buf = this.read_dst.check_and_to_rawbuf(ptr, len);
        if !this.read_fut.armed() {
            // we must leak the stream
            let stream = unsafe { &mut *this.stream.get() };
            this.read_fut
                .arm_future(AsyncReadRent::read(stream, raw_buf));
        }

        let (ret, _) = match this.read_fut.poll(cx) {
            std::task::Poll::Ready(r) => r,
            std::task::Poll::Pending => {
                return std::task::Poll::Pending;
            }
        };
        this.read_dst.reset();
        buf.advance(ret?);
        std::task::Poll::Ready(Ok(()))
    }
}

impl tokio::io::AsyncWrite for TcpStreamCompat {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        let (ptr, len) = (buf.as_ptr(), buf.len());

        // Set or check write_dst
        // Note: the check can not prevent memory crash when user misuse it.
        let raw_buf = this.write_dst.check_and_to_rawbuf(ptr, len);
        if !this.write_fut.armed() {
            // we must leak the stream
            let stream = unsafe { &mut *this.stream.get() };
            this.write_fut
                .arm_future(AsyncWriteRent::write(stream, raw_buf));
        }

        let (ret, _) = match this.write_fut.poll(cx) {
            std::task::Poll::Ready(r) => r,
            std::task::Poll::Pending => {
                return std::task::Poll::Pending;
            }
        };
        this.write_dst.reset();
        std::task::Poll::Ready(ret)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        if !this.flush_fut.armed() {
            let stream = unsafe { &mut *this.stream.get() };
            this.flush_fut.arm_future(stream.flush());
        }
        this.flush_fut.poll(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        if !this.shutdown_fut.armed() {
            let stream = unsafe { &mut *this.stream.get() };
            this.shutdown_fut.arm_future(stream.shutdown());
        }
        this.shutdown_fut.poll(cx)
    }
}
