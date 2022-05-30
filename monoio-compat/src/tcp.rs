use std::{future::Future, pin::Pin};

use crate::{AsyncRead, AsyncWrite};
use monoio::buf::{IoBuf, IoBufMut};
use monoio::io::{AsyncReadRent, AsyncWriteRent};
use monoio::net::TcpStream;

type PinnedFuture<B> = Pin<Box<dyn Future<Output = monoio::BufResult<usize, B>> + 'static>>;

struct LimitedBuffer {
    buf: Vec<u8>,
    limit: usize,
}

unsafe impl IoBuf for LimitedBuffer {
    fn read_ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }

    fn bytes_init(&self) -> usize {
        self.buf.len().min(self.limit)
    }
}

unsafe impl IoBufMut for LimitedBuffer {
    fn write_ptr(&mut self) -> *mut u8 {
        self.buf.as_mut_ptr()
    }

    fn bytes_total(&self) -> usize {
        self.buf.capacity().min(self.limit)
    }

    unsafe fn set_init(&mut self, pos: usize) {
        self.buf.set_init(pos);
    }
}

pub struct TcpStreamCompat {
    stream: TcpStream,
    read_fut: Option<PinnedFuture<LimitedBuffer>>,
    read_owned_buf: Option<Vec<u8>>,
    write_fut: Option<PinnedFuture<Vec<u8>>>,
    write_owned_buf: Option<Vec<u8>>,
    write_len: usize,
}

impl TcpStreamCompat {
    /// Creates a new `TcpStreamCompat` from a monoio `TcpStream`.
    ///
    /// # Safety
    /// User must ensure that the data slice is the same between `poll_write`
    /// and `poll_read` if the last call returns `Pending`.
    pub unsafe fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            read_fut: None,
            read_owned_buf: Some(Vec::with_capacity(1024)),
            write_fut: None,
            write_owned_buf: Some(Vec::with_capacity(1024)),
            write_len: 0,
        }
    }
}

impl From<TcpStreamCompat> for TcpStream {
    fn from(stream: TcpStreamCompat) -> Self {
        stream.stream
    }
}

#[allow(clippy::cast_ref_to_mut)]
impl AsyncRead for TcpStreamCompat {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let mut fut = this.read_fut.take().unwrap_or_else(|| {
            // we can read at most that length
            let mut owned_buf = this.read_owned_buf.take().unwrap();
            let stream = unsafe { &mut *(&this.stream as *const TcpStream as *mut TcpStream) };

            let cap = owned_buf.capacity();
            let remaining = buf.remaining();
            if remaining > cap {
                owned_buf.reserve(remaining - cap);
            }

            let limited_buffer = LimitedBuffer {
                buf: owned_buf,
                limit: remaining,
            };

            let f = AsyncReadRent::read(stream, limited_buffer);
            Box::pin(f)
        });
        match fut.as_mut().poll(cx) {
            std::task::Poll::Ready((r, limited_buffer)) => {
                let inner = limited_buffer.buf;
                let ret = if let Err(e) = r {
                    std::task::Poll::Ready(Err(e))
                } else {
                    buf.put_slice(&inner);
                    std::task::Poll::Ready(Ok(()))
                };
                this.read_owned_buf = Some(inner);
                ret
            }
            std::task::Poll::Pending => {
                this.read_fut = Some(fut);
                std::task::Poll::Pending
            }
        }
    }
}

#[allow(clippy::cast_ref_to_mut)]
impl AsyncWrite for TcpStreamCompat {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        let mut fut = this.write_fut.take().unwrap_or_else(|| {
            let mut owned_buf = this.write_owned_buf.take().unwrap();
            let stream = unsafe { &mut *(&this.stream as *const TcpStream as *mut TcpStream) };

            owned_buf.clear();
            owned_buf.extend_from_slice(buf);

            this.write_len = owned_buf.len();
            let f = AsyncWriteRent::write(stream, owned_buf);
            Box::pin(f)
        });
        // Check if the slice between different poll_write calls is the same
        if buf.len() != this.write_len {
            panic!("write slice length mismatch between poll_write");
        }
        match fut.as_mut().poll(cx) {
            std::task::Poll::Ready((r, owned_buf)) => {
                this.write_owned_buf = Some(owned_buf);
                match r {
                    Ok(n) => std::task::Poll::Ready(Ok(n)),
                    Err(e) => std::task::Poll::Ready(Err(e)),
                }
            }
            std::task::Poll::Pending => {
                this.write_fut = Some(fut);
                std::task::Poll::Pending
            }
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        let mut fut = this.stream.shutdown();

        // Our shutdown is not a real future, so here we do not save it.
        unsafe { Pin::new_unchecked(&mut fut).poll(cx) }
    }
}

#[cfg(test)]
mod tests {
    use crate::{AsyncReadExt, AsyncWriteExt, TcpStreamCompat};

    #[monoio::test_all]
    async fn test_rw() {
        let listener = monoio::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = async move {
            let (conn, _) = listener.accept().await.unwrap();
            let mut compat_conn: TcpStreamCompat = unsafe { TcpStreamCompat::new(conn) };

            let mut buf = [0u8; 10];
            compat_conn.read_exact(&mut buf).await.unwrap();
            buf[0] += 1;
            compat_conn.write_all(&buf).await.unwrap();
        };
        let client = async {
            let conn = monoio::net::TcpStream::connect(addr).await.unwrap();
            let mut compat_conn: TcpStreamCompat = unsafe { TcpStreamCompat::new(conn) };

            let mut buf = [65u8; 10];
            compat_conn.write_all(&buf).await.unwrap();
            compat_conn.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf[0], 66);
        };
        monoio::spawn(server);
        client.await;
    }
}
