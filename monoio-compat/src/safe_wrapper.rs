use std::io;

use monoio::{
    buf::IoBufMut,
    io::{AsyncReadRent, AsyncWriteRent, AsyncWriteRentExt, Split},
    BufResult,
};

use crate::{box_future::MaybeArmedBoxFuture, buf::Buf};

/// A wrapper for stream with ownership that impl AsyncReadRent and AsyncWriteRent.
/// The Wrapper will impl tokio AsyncRead and AsyncWrite.
/// Mainly used for compatible.
pub struct StreamWrapper<T> {
    stream: T,
    read_buf: Option<Buf>,
    write_buf: Option<Buf>,

    read_fut: MaybeArmedBoxFuture<BufResult<usize, Buf>>,
    write_fut: MaybeArmedBoxFuture<BufResult<usize, Buf>>,
    flush_fut: MaybeArmedBoxFuture<io::Result<()>>,
    shutdown_fut: MaybeArmedBoxFuture<io::Result<()>>,
}

unsafe impl<T: Split> Split for StreamWrapper<T> {}

impl<T> StreamWrapper<T> {
    /// Consume self and get inner T.
    pub fn into_inner(self) -> T {
        self.stream
    }

    /// Creates a new `TcpStreamCompat` from a monoio `TcpStream` or `UnixStream`.
    pub fn new_with_buffer_size(stream: T, read_buffer: usize, write_buffer: usize) -> Self {
        let r_buf = Buf::new(read_buffer);
        let w_buf = Buf::new(write_buffer);

        Self {
            stream,
            read_buf: Some(r_buf),
            write_buf: Some(w_buf),
            read_fut: Default::default(),
            write_fut: Default::default(),
            flush_fut: Default::default(),
            shutdown_fut: Default::default(),
        }
    }

    /// Creates a new `TcpStreamCompat` from a monoio `TcpStream` or `UnixStream`.
    pub fn new(stream: T) -> Self {
        const DEFAULT_READ_BUFFER: usize = 8 * 1024;
        const DEFAULT_WRITE_BUFFER: usize = 8 * 1024;
        Self::new_with_buffer_size(stream, DEFAULT_READ_BUFFER, DEFAULT_WRITE_BUFFER)
    }
}

impl<T: AsyncReadRent + Unpin + 'static> tokio::io::AsyncRead for StreamWrapper<T> {
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
                let stream = unsafe { &mut *(&this.stream as *const T as *mut T) };
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

impl<T: AsyncWriteRent + Unpin + 'static> tokio::io::AsyncWrite for StreamWrapper<T> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        if buf.is_empty() {
            return std::task::Poll::Ready(Ok(0));
        }
        let this = self.get_mut();

        // if there is some future armed, we must poll it until ready.
        // if it returns error, we will return it;
        // if it returns ok, we ignore it.
        if this.write_fut.armed() {
            let (ret, mut owned_buf) = match this.write_fut.poll(cx) {
                std::task::Poll::Ready(r) => r,
                std::task::Poll::Pending => {
                    return std::task::Poll::Pending;
                }
            };
            // clear the buffer
            unsafe { owned_buf.set_init(0) };
            this.write_buf = Some(owned_buf);
            if ret.is_err() {
                return std::task::Poll::Ready(ret);
            }
        }

        // now we should arm it again.
        // we will copy the data and return Ready.
        // Though return Ready does not mean really ready, but this helps preventing
        // poll_write different data.

        // # Safety
        // We always make sure the write_buf is Some.
        let mut owned_buf = unsafe { this.write_buf.take().unwrap_unchecked() };
        let owned_buf_mut = owned_buf.buf_to_write();
        let len = buf.len().min(owned_buf_mut.len());
        // # Safety
        // We can make sure the buf and buf_mut_slice have len size data.
        unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), owned_buf_mut.as_mut_ptr(), len) };
        unsafe { owned_buf.set_init(len) };

        // we must leak the stream
        #[allow(clippy::cast_ref_to_mut)]
        let stream = unsafe { &mut *(&this.stream as *const T as *mut T) };
        this.write_fut
            .arm_future(AsyncWriteRentExt::write_all(stream, owned_buf));
        match this.write_fut.poll(cx) {
            std::task::Poll::Ready((ret, mut buf)) => {
                unsafe { buf.set_init(0) };
                this.write_buf = Some(buf);
                if ret.is_err() {
                    return std::task::Poll::Ready(ret);
                }
            }
            std::task::Poll::Pending => (),
        }
        // if there is no error, no matter it is sending or sent, we will
        // return Ready.
        std::task::Poll::Ready(Ok(len))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        if this.write_fut.armed() {
            match this.write_fut.poll(cx) {
                std::task::Poll::Ready((ret, mut buf)) => {
                    unsafe { buf.set_init(0) };
                    this.write_buf = Some(buf);
                    if let Err(e) = ret {
                        return std::task::Poll::Ready(Err(e));
                    }
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }

        if !this.flush_fut.armed() {
            #[allow(clippy::cast_ref_to_mut)]
            let stream = unsafe { &mut *(&this.stream as *const T as *mut T) };
            this.flush_fut.arm_future(stream.flush());
        }
        this.flush_fut.poll(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();

        if this.write_fut.armed() {
            match this.write_fut.poll(cx) {
                std::task::Poll::Ready((ret, mut buf)) => {
                    unsafe { buf.set_init(0) };
                    this.write_buf = Some(buf);
                    if let Err(e) = ret {
                        return std::task::Poll::Ready(Err(e));
                    }
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }

        if !this.shutdown_fut.armed() {
            #[allow(clippy::cast_ref_to_mut)]
            let stream = unsafe { &mut *(&this.stream as *const T as *mut T) };
            this.shutdown_fut.arm_future(stream.shutdown());
        }
        this.shutdown_fut.poll(cx)
    }
}
