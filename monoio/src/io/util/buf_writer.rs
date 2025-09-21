use std::{future::Future, io};

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut, IoVecWrapper, Slice},
    io::{AsyncBufRead, AsyncReadRent, AsyncWriteRent, AsyncWriteRentExt},
    BufResult,
};

/// BufWriter is a struct with a buffer. BufWriter implements AsyncWriteRent,
/// and if the inner io implements AsyncReadRent, it will delegate the
/// implementation.
pub struct BufWriter<W> {
    inner: W,
    buf: Option<Box<[u8]>>,
    cap: usize,
}

const DEFAULT_BUF_SIZE: usize = 8 * 1024;

impl<W> BufWriter<W> {
    /// Create BufWriter with default buffer size
    #[inline]
    pub fn new(inner: W) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Create BufWriter with given buffer size
    #[inline]
    pub fn with_capacity(capacity: usize, inner: W) -> Self {
        let buffer = vec![0; capacity];
        Self {
            inner,
            buf: Some(buffer.into_boxed_slice()),
            cap: 0,
        }
    }

    /// Gets a reference to the underlying writer.
    #[inline]
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Gets a mutable reference to the underlying writer.
    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Consumes this `BufWriter`, returning the underlying writer.
    ///
    /// Note that any leftover data in the internal buffer is lost.
    #[inline]
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Returns a reference to the internally buffered data.
    #[inline]
    pub fn buffer(&self) -> &[u8] {
        &self.buf.as_ref().expect("unable to take buffer")[..self.cap]
    }

    /// Invalidates all data in the internal buffer.
    #[inline]
    fn discard_buffer(&mut self) {
        self.cap = 0;
    }
}

impl<W: AsyncWriteRent> BufWriter<W> {
    async fn flush_buf(&mut self) -> io::Result<()> {
        if self.cap != 0 {
            // there is some data left inside internal buf
            let buf = self
                .buf
                .take()
                .expect("no buffer available, generated future must be awaited");
            // move buf to slice and write_all
            let slice = Slice::new(buf, 0, self.cap);
            let (ret, slice) = self.inner.write_all(slice).await;
            // move it back and return
            self.buf = Some(slice.into_inner());
            ret?;
            self.discard_buffer();
        }
        Ok(())
    }
}

impl<W: AsyncWriteRent> AsyncWriteRent for BufWriter<W> {
    async fn write<T: IoBuf>(&mut self, buf: T) -> BufResult<usize, T> {
        let owned_buf = self.buf.as_ref().unwrap();
        let owned_len = owned_buf.len();
        let amt = buf.bytes_init();

        if self.cap + amt > owned_len {
            // Buf can not be copied directly into OwnedBuf,
            // we must flush OwnedBuf first.
            match self.flush_buf().await {
                Ok(_) => (),
                Err(e) => {
                    return (Err(e), buf);
                }
            }
        }

        // Now there are two situations here:
        // 1. OwnedBuf has data, and self.cap + amt <= owned_len,
        // which means the data can be copied into OwnedBuf.
        // 2. OwnedBuf is empty. If we can copy buf into OwnedBuf,
        // we will copy it, otherwise we will send it directly(in
        // this situation, the OwnedBuf must be already empty).
        if amt > owned_len {
            self.inner.write(buf).await
        } else {
            unsafe {
                let owned_buf = self.buf.as_mut().unwrap();
                owned_buf
                    .as_mut_ptr()
                    .add(self.cap)
                    .copy_from_nonoverlapping(buf.read_ptr(), amt);
            }
            self.cap += amt;
            (Ok(amt), buf)
        }
    }

    // TODO: implement it as real io_vec
    async fn writev<T: IoVecBuf>(&mut self, buf: T) -> BufResult<usize, T> {
        let slice = match IoVecWrapper::new(buf) {
            Ok(slice) => slice,
            Err(buf) => return (Ok(0), buf),
        };

        let (result, slice) = self.write(slice).await;
        (result, slice.into_inner())
    }

    async fn flush(&mut self) -> std::io::Result<()> {
        self.flush_buf().await?;
        self.inner.flush().await
    }

    async fn shutdown(&mut self) -> std::io::Result<()> {
        self.flush_buf().await?;
        self.inner.shutdown().await
    }
}

impl<W: AsyncWriteRent + AsyncReadRent> AsyncReadRent for BufWriter<W> {
    #[inline]
    fn read<T: IoBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        self.inner.read(buf)
    }

    #[inline]
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        self.inner.readv(buf)
    }
}

impl<W: AsyncWriteRent + AsyncBufRead> AsyncBufRead for BufWriter<W> {
    #[inline]
    fn fill_buf(&mut self) -> impl Future<Output = std::io::Result<&[u8]>> {
        self.inner.fill_buf()
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }
}
