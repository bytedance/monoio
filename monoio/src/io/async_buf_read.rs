use crate::{
    buf::RawBuf,
    io::{AsyncReadRent, AsyncWriteRent},
};
use std::future::Future;

/// AsyncBufRead: async read with buffered content
pub trait AsyncBufRead: AsyncReadRent {
    /// The returned future of fill_buf
    type FillBufFuture<'a>: Future<Output = std::io::Result<&'a [u8]>>
    where
        Self: 'a;

    /// Try read data and get a reference to the internal buffer
    fn fill_buf(&mut self) -> Self::FillBufFuture<'_>;
    /// Mark how much data is read
    fn consume(&mut self, amt: usize);
}

/// BufReader is a struct with a buffer that implement AsyncBufRead.
pub struct BufReader<R> {
    inner: R,
    buf: Option<Box<[u8]>>,
    pos: usize,
    cap: usize,
}

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

impl<R> BufReader<R> {
    /// Create BufReader with default buffer size
    pub fn new(inner: R) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Create BufReader with given buffer size
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
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Returns a reference to the internally buffered data.
    ///
    /// Unlike `fill_buf`, this will not attempt to fill the buffer if it is empty.
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
    type ReadFuture<'a, T>  = impl std::future::Future<Output = crate::BufResult<usize, T>> where
        T: 'a, R: 'a;

    type ReadvFuture<'a, T>= impl std::future::Future<Output = crate::BufResult<usize, T>> where
        T: 'a, R: 'a;

    fn read<T: crate::buf::IoBufMut>(&mut self, mut buf: T) -> Self::ReadFuture<'_, T> {
        async move {
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
            (Ok(0), buf)
        }
    }

    fn readv<T: crate::buf::IoVecBufMut>(&mut self, mut buf: T) -> Self::ReadvFuture<'_, T> {
        async move {
            let n = match unsafe { RawBuf::new_from_iovec_mut(&mut buf) } {
                Some(raw_buf) => self.read(raw_buf).await.0,
                None => Ok(0),
            };
            if let Ok(n) = n {
                unsafe { buf.set_init(n) };
            }
            (n, buf)
        }
    }
}

impl<R: AsyncReadRent> AsyncBufRead for BufReader<R> {
    type FillBufFuture<'a> = impl Future<Output = std::io::Result<&'a [u8]>> where Self: 'a;

    fn fill_buf(&mut self) -> Self::FillBufFuture<'_> {
        async {
            if self.pos >= self.cap {
                // there's no buffered data
                debug_assert!(self.pos == self.cap);
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
    }

    fn consume(&mut self, amt: usize) {
        self.pos = self.cap.min(self.pos + amt);
    }
}

impl<R: AsyncReadRent + AsyncWriteRent> AsyncWriteRent for BufReader<R> {
    type WriteFuture<'a, T> = impl std::future::Future<Output = crate::BufResult<usize, T>> where
    T: 'a, R: 'a;

    type WritevFuture<'a, T>= impl std::future::Future<Output = crate::BufResult<usize, T>> where
    T: 'a, R: 'a;

    type ShutdownFuture<'a> = impl Future<Output = Result<(), std::io::Error>> where R: 'a;

    fn write<T: crate::buf::IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        self.inner.write(buf)
    }

    fn writev<T: crate::buf::IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        self.inner.writev(buf_vec)
    }

    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        self.inner.shutdown()
    }
}
