use std::{future::Future, pin::Pin};

use crate::{AsyncRead, AsyncWrite};
use monoio::io::{AsyncReadRent, AsyncWriteRent};
use monoio::net::TcpStream;

type PinnedFuture<B> = Pin<Box<dyn Future<Output = monoio::BufResult<usize, B>> + 'static>>;
pub struct TcpStreamCompat {
    stream: TcpStream,
    read_fut: Option<PinnedFuture<Vec<u8>>>,
    read_owned_buf: Option<Vec<u8>>,
    write_fut: Option<PinnedFuture<Vec<u8>>>,
    write_owned_buf: Option<Vec<u8>>,
}

impl From<TcpStream> for TcpStreamCompat {
    fn from(stream: TcpStream) -> Self {
        Self {
            stream,
            read_fut: None,
            read_owned_buf: Some(Vec::with_capacity(1024)),
            write_fut: None,
            write_owned_buf: Some(Vec::with_capacity(1024)),
        }
    }
}

impl From<TcpStreamCompat> for TcpStream {
    fn from(stream: TcpStreamCompat) -> Self {
        stream.stream
    }
}

impl AsyncRead for TcpStreamCompat {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let mut fut = this.read_fut.take().unwrap_or_else(|| {
            let owned_buf = this.read_owned_buf.take().unwrap();
            let stream = unsafe { &*(&this.stream as *const TcpStream) };
            let f = AsyncReadRent::read(stream, owned_buf);
            Box::pin(f)
        });
        match fut.as_mut().poll(cx) {
            std::task::Poll::Ready((r, owned_buf)) => {
                let ret = if let Err(e) = r {
                    std::task::Poll::Ready(Err(e))
                } else {
                    buf.put_slice(&owned_buf);
                    std::task::Poll::Ready(Ok(()))
                };
                this.read_owned_buf = Some(owned_buf);
                ret
            }
            std::task::Poll::Pending => {
                this.read_fut = Some(fut);
                std::task::Poll::Pending
            }
        }
    }
}

impl AsyncWrite for TcpStreamCompat {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        let mut fut = this.write_fut.take().unwrap_or_else(|| {
            let mut owned_buf = this.write_owned_buf.take().unwrap();
            let stream = unsafe { &*(&this.stream as *const TcpStream) };

            owned_buf.clear();
            owned_buf.extend_from_slice(buf);

            let f = AsyncWriteRent::write(stream, owned_buf);
            Box::pin(f)
        });
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
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{AsyncReadExt, AsyncWriteExt, TcpStreamCompat};

    #[monoio::test]
    async fn test_rw() {
        const ADDRESS: &str = "127.0.0.1:50009";
        let (mut oneshot_tx, mut oneshot_rx) = local_sync::oneshot::channel::<()>();

        let server = async move {
            let listener = monoio::net::TcpListener::bind(ADDRESS).unwrap();
            oneshot_rx.close();
            let (conn, _) = listener.accept().await.unwrap();
            let mut compat_conn: TcpStreamCompat = conn.into();

            let mut buf = [0u8; 10];
            compat_conn.read_exact(&mut buf).await.unwrap();
            buf[0] += 1;
            compat_conn.write_all(&buf).await.unwrap();
        };
        let client = async {
            oneshot_tx.closed().await;
            let conn = monoio::net::TcpStream::connect(ADDRESS).await.unwrap();
            let mut compat_conn: TcpStreamCompat = conn.into();

            let mut buf = [65u8; 10];
            compat_conn.write_all(&buf).await.unwrap();
            compat_conn.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf[0], 66);
        };
        monoio::join!(client, server);
    }
}
