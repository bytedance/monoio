use std::io;

use monoio::buf::IoBufMut;
use monoio::io::AsyncWriteRent;
use monoio::BufResult;
use monoio::{io::AsyncReadRent, net::TcpStream};

use crate::box_future::MaybeArmedBoxFuture;
use crate::buf::Buf;

pub struct TcpStreamCompat {
    stream: TcpStream,
    read_buf: Option<Buf>,
    write_buf: Option<Buf>,

    read_fut: MaybeArmedBoxFuture<BufResult<usize, Buf>>,
    write_fut: MaybeArmedBoxFuture<BufResult<usize, Buf>>,
    flush_fut: MaybeArmedBoxFuture<io::Result<()>>,
    shutdown_fut: MaybeArmedBoxFuture<io::Result<()>>,

    // used for checking
    last_write_len: usize,
}

impl From<TcpStreamCompat> for TcpStream {
    fn from(stream: TcpStreamCompat) -> Self {
        stream.stream
    }
}

impl TcpStreamCompat {
    /// Creates a new `TcpStreamCompat` from a monoio `TcpStream`.
    ///
    /// # Safety
    /// User must ensure that the data slices provided to `poll_write`
    /// before Poll::Ready are with the same data.
    pub unsafe fn new(stream: TcpStream) -> Self {
        let r_buf = Buf::new(8 * 1024);
        let w_buf = Buf::new(8 * 1024);

        Self {
            stream,
            read_buf: Some(r_buf),
            write_buf: Some(w_buf),
            read_fut: Default::default(),
            write_fut: Default::default(),
            flush_fut: Default::default(),
            shutdown_fut: Default::default(),
            last_write_len: 0,
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

        loop {
            // if the future not armed, this means maybe buffer has data.
            if !this.read_fut.armed() {
                // if there is some data left in our buf, copy it and return.
                let read_buf_mut = unsafe { this.read_buf.as_mut().unwrap_unchecked() };
                if !read_buf_mut.is_empty() {
                    // copy directly from inner buf to buf
                    let our_buf = read_buf_mut.buf_to_read(buf.remaining());
                    let our_buf_len = our_buf.len();
                    buf.put_slice(our_buf);
                    unsafe { read_buf_mut.advance_offset(our_buf_len) };
                    return std::task::Poll::Ready(Ok(()));
                }

                // there is no data in buffer. we will construct the future
                let buf = unsafe { this.read_buf.take().unwrap_unchecked() };
                // we must leak the stream
                #[allow(clippy::cast_ref_to_mut)]
                let stream = unsafe { &mut *(&this.stream as *const TcpStream as *mut TcpStream) };
                this.read_fut.arm_future(AsyncReadRent::read(stream, buf));
            }

            // the future slot is armed now. we will poll it.
            let (ret, buf) = match this.read_fut.poll(cx) {
                std::task::Poll::Ready(out) => out,
                std::task::Poll::Pending => {
                    return std::task::Poll::Pending;
                }
            };
            this.read_buf = Some(buf);
            if ret? == 0 {
                // on eof, return directly; otherwise goto next loop.
                return std::task::Poll::Ready(Ok(()));
            }
        }
    }
}

impl tokio::io::AsyncWrite for TcpStreamCompat {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();

        // if there is no write future armed, we will copy the data and construct it
        if !this.write_fut.armed() {
            let mut owned_buf = unsafe { this.write_buf.take().unwrap_unchecked() };
            let owned_buf_mut = owned_buf.buf_to_write();
            let len = buf.len().min(owned_buf_mut.len());
            owned_buf_mut[..len].copy_from_slice(&buf[..len]);
            unsafe { owned_buf.set_init(len) };
            this.last_write_len = len;

            // we must leak the stream
            #[allow(clippy::cast_ref_to_mut)]
            let stream = unsafe { &mut *(&this.stream as *const TcpStream as *mut TcpStream) };
            this.write_fut
                .arm_future(AsyncWriteRent::write(stream, owned_buf));
        }

        // Check if the slice between different poll_write calls is the same
        if buf.len() != this.last_write_len {
            panic!("write slice length mismatch between poll_write");
        }
        // the future must be armed
        let (ret, owned_buf) = match this.write_fut.poll(cx) {
            std::task::Poll::Ready(r) => r,
            std::task::Poll::Pending => {
                return std::task::Poll::Pending;
            }
        };
        this.write_buf = Some(owned_buf);
        std::task::Poll::Ready(ret)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        if !this.flush_fut.armed() {
            #[allow(clippy::cast_ref_to_mut)]
            let stream = unsafe { &mut *(&this.stream as *const TcpStream as *mut TcpStream) };
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
            #[allow(clippy::cast_ref_to_mut)]
            let stream = unsafe { &mut *(&this.stream as *const TcpStream as *mut TcpStream) };
            this.shutdown_fut.arm_future(stream.shutdown());
        }
        this.shutdown_fut.poll(cx)
    }
}
