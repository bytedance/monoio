use std::future::Future;

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut, IoVecWrapperMut},
    io::{AsyncBufRead, AsyncReadRent, AsyncWriteRent},
    BufResult,
};

enum BufState {
    /// This buffer is never used, in use, or used by a previously cancelled
    /// read.
    Unallocated(usize),

    /// This buffer is available.
    Available(Box<[u8]>),
}

impl BufState {
    fn take(&mut self) -> Box<[u8]> {
        let size = self.size();
        match std::mem::replace(self, BufState::Unallocated(size)) {
            BufState::Unallocated(len) => vec![0u8; len].into(),
            BufState::Available(buf) => buf,
        }
    }

    fn size(&self) -> usize {
        match self {
            BufState::Unallocated(len) => *len,
            BufState::Available(buf) => buf.len(),
        }
    }
}

/// BufReader is a struct with a buffer. BufReader implements AsyncBufRead
/// and AsyncReadRent, and if the inner io implements AsyncWriteRent, it
/// will delegate the implementation.
pub struct BufReader<R> {
    inner: R,
    buf: BufState,
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
        Self {
            inner,
            buf: BufState::Unallocated(capacity),
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

    /// Consumes this `BufReader`, returning the underlying reader.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Returns a reference to the internally buffered data.
    ///
    /// Unlike `fill_buf`, this will not attempt to fill the buffer if it is
    /// empty.
    pub fn buffer(&self) -> &[u8] {
        match &self.buf {
            BufState::Unallocated(_) => &[],
            BufState::Available(buf) => &buf[self.pos..self.cap],
        }
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
        if self.pos == self.cap && buf.bytes_total() >= self.buf.size() {
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
            let buf = self.buf.take();
            let (res, buf_) = self.inner.read(buf).await;
            self.buf = BufState::Available(buf_);
            match res {
                Ok(n) => {
                    self.pos = 0;
                    self.cap = n;
                    return Ok(match &self.buf {
                        BufState::Available(buf) => &buf[self.pos..self.cap],
                        BufState::Unallocated(_) => {
                            // We just put the buf into Option, so it must be Some.
                            unreachable!()
                        }
                    });
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        match &self.buf {
            BufState::Available(buf) => Ok(&buf[self.pos..self.cap]),
            BufState::Unallocated(_) => {
                // The `Unallocated` state only happens if:
                // - nothing is read into this `BufReader` yet (pos == 0, cap == 0), or
                // - a previous `fill_buf` was cancelled (pos == cap)
                // Both cases are covered by the above `if` block, so it's impossible
                // to reach here.
                unreachable!("buf is unallocated");
            }
        }
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
