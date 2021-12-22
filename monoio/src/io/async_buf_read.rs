use futures::Future;

use super::AsyncReadRent;

/// AsyncBufRead: async read with buffered content
pub trait AsyncBufRead {
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
}

impl<R> AsyncBufRead for BufReader<R>
where
    R: AsyncReadRent,
{
    type FillBufFuture<'a>
    where
        Self: 'a,
    = impl Future<Output = std::io::Result<&'a [u8]>>;

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
