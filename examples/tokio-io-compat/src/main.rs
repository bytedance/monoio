//! HTTP client example with hyper in compatible mode(without cost).
//!
//! It will try to fetch https://www.bytedance.com/ and print the
//! response.
//!
//! It looks like the `hyper_client.rs` in example. The difference is
//! in this version the tokio AsyncRead and AsyncWrite is implemented
//! without additional cost because we only enable legacy feature.

use std::{future::Future, pin::Pin};

#[derive(Clone)]
struct HyperExecutor;

impl<F> hyper::rt::Executor<F> for HyperExecutor
where
    F: Future + 'static,
    F::Output: 'static,
{
    #[inline]
    fn execute(&self, fut: F) {
        monoio::spawn(fut);
    }
}

#[derive(Clone)]
struct HyperConnector;

impl tower_service::Service<hyper::Uri> for HyperConnector {
    type Response = HyperConnection;

    type Error = std::io::Error;

    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    #[inline]
    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: hyper::Uri) -> Self::Future {
        let host = uri.host().unwrap();
        let port = uri.port_u16().unwrap_or(80);
        let address = format!("{host}:{port}");

        #[allow(clippy::type_complexity)]
        let b: Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>> =
            Box::pin(async move {
                let conn = monoio::net::TcpStream::connect(address).await?;
                let hyper_conn = HyperConnection(conn);
                Ok(hyper_conn)
            });
        // Use transmust to make future Send(infact it is not)
        unsafe { std::mem::transmute(b) }
    }
}

struct HyperConnection(monoio::net::TcpStream);

impl tokio::io::AsyncRead for HyperConnection {
    #[inline]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for HyperConnection {
    #[inline]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl hyper::client::connect::Connection for HyperConnection {
    #[inline]
    fn connected(&self) -> hyper::client::connect::Connected {
        hyper::client::connect::Connected::new()
    }
}

#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for HyperConnection {}

#[monoio::main]
async fn main() {
    println!("Running http client");
    let connector = HyperConnector;
    let client = hyper::Client::builder()
        .executor(HyperExecutor)
        .build::<HyperConnector, hyper::Body>(connector);
    let res = client
        .get("https://www.bytedance.com/".parse().unwrap())
        .await
        .expect("failed to fetch");
    println!("Response status: {}", res.status());
    let body = hyper::body::to_bytes(res.into_body())
        .await
        .expect("failed to read body");
    let body =
        String::from_utf8(body.into_iter().collect()).expect("failed to convert body to string");
    println!("Response body: {body}");
}
