use std::future::Future;

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut, IoVecWrapperMut},
    io::{AsyncBufRead, AsyncReadRent, AsyncWriteRent},
    BufResult,
};

/// BufReader is a struct with a buffer. BufReader implements AsyncBufRead
/// and AsyncReadRent, and if the inner io implements AsyncWriteRent, it
/// will delegate the implementation.
pub struct BufReader<R> {
    inner: R,
    buf: Option<Box<[u8]>>,
    pos: usize,
    cap: usize,
}

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

impl<R> BufReader<R> {
    /// Create BufReader with default buffer size
    #[inline]
    pub fn new(inner: R) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Create BufReader with given buffer size
    #[inline]
    pub fn with_capacity(capacity: usize, inner: R) -> Self {
        let buffer = vec![0; capacity];
        Self {
            inner,
            buf: Some(buffer.into_boxed_slice()),
            pos: 0,
            cap: 0,
        }
    }

    /// Gets a reference to the underlying reader.
    ///
    /// It is inadvisable to directly read from the underlying reader.
    #[inline]
    pub const fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Consumes this `BufReader`, returning the underlying reader.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    #[inline]
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Returns a reference to the internally buffered data.
    ///
    /// Unlike `fill_buf`, this will not attempt to fill the buffer if it is
    /// empty.
    #[inline]
    pub fn buffer(&self) -> &[u8] {
        &self.buf.as_ref().expect("unable to take buffer")[self.pos..self.cap]
    }

    /// Invalidates all data in the internal buffer.
    #[inline]
    fn discard_buffer(&mut self) {
        self.pos = 0;
        self.cap = 0;
    }
}

impl<R: AsyncReadRent> AsyncReadRent for BufReader<R> {
    async fn read<T: IoBufMut>(&mut self, mut buf: T) -> BufResult<usize, T> {
        // If we don't have any buffered data and we're doing a massive read
        // (larger than our internal buffer), bypass our internal buffer
        // entirely.
        let owned_buf = self.buf.as_ref().unwrap();
        if self.pos == self.cap && buf.bytes_total() >= owned_buf.len() {
            self.discard_buffer();
            return self.inner.read(buf).await;
        }

        let rem = match self.fill_buf().await {
            Ok(slice) => slice,
            Err(e) => {
                return (Err(e), buf);
            }
        };
        let amt = std::cmp::min(rem.len(), buf.bytes_total());
        unsafe {
            buf.write_ptr().copy_from_nonoverlapping(rem.as_ptr(), amt);
            buf.set_init(amt);
        }
        self.consume(amt);
        (Ok(amt), buf)
    }

    async fn readv<T: IoVecBufMut>(&mut self, mut buf: T) -> BufResult<usize, T> {
        let slice = match IoVecWrapperMut::new(buf) {
            Ok(slice) => slice,
            Err(buf) => return (Ok(0), buf),
        };

        let (result, slice) = self.read(slice).await;
        buf = slice.into_inner();
        if let Ok(n) = result {
            unsafe { buf.set_init(n) };
        }
        (result, buf)
    }
}

impl<R: AsyncReadRent> AsyncBufRead for BufReader<R> {
    async fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.pos == self.cap {
            // there's no buffered data
            let buf = self
                .buf
                .take()
                .expect("no buffer available, generated future must be awaited");
            let (res, buf_) = self.inner.read(buf).await;
            self.buf = Some(buf_);
            match res {
                Ok(n) => {
                    self.pos = 0;
                    self.cap = n;
                    return Ok(unsafe {
                        // We just put the buf into Option, so it must be Some.
                        &(self.buf.as_ref().unwrap_unchecked().as_ref())[self.pos..self.cap]
                    });
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(&(self
            .buf
            .as_ref()
            .expect("no buffer available, generated future must be awaited")
            .as_ref())[self.pos..self.cap])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = self.cap.min(self.pos + amt);
    }
}

impl<R: AsyncReadRent + AsyncWriteRent> AsyncWriteRent for BufReader<R> {
    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        self.inner.write(buf)
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        self.inner.writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>> {
        self.inner.flush()
    }

    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>> {
        self.inner.shutdown()
    }
}
