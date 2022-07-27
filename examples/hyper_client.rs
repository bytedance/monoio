//! HTTP client example with hyper in compatible mode.
//!
//! It will try to fetch http://127.0.0.1:23300/monoio and print the
//! response.
//!
//! Note:
//! It is not recommended to use this example as a production code.
//! The `hyper` require `Send` for a future and obviously the future
//! is not `Send` in monoio. So we just use some unsafe code to let
//! it pass which infact not a good solution but the only way to
//! make it work without modifying hyper.

use std::{future::Future, pin::Pin};

use monoio_compat::TcpStreamCompat;

#[derive(Clone)]
struct HyperExecutor;

impl<F> hyper::rt::Executor<F> for HyperExecutor
where
    F: Future + 'static,
    F::Output: 'static,
{
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

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: hyper::Uri) -> Self::Future {
        let host = uri.host().unwrap();
        let port = uri.port_u16().unwrap_or(80);
        let address = format!("{}:{}", host, port);

        #[allow(clippy::type_complexity)]
        let b: Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>> =
            Box::pin(async move {
                let conn = monoio::net::TcpStream::connect(address).await?;
                let hyper_conn = HyperConnection(unsafe { TcpStreamCompat::new(conn) });
                Ok(hyper_conn)
            });
        unsafe { std::mem::transmute(b) }
    }
}

struct HyperConnection(TcpStreamCompat);

impl tokio::io::AsyncRead for HyperConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for HyperConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl hyper::client::connect::Connection for HyperConnection {
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
        .get("http://127.0.0.1:23300/monoio".parse().unwrap())
        .await
        .expect("failed to fetch");
    println!("Response status: {}", res.status());
    let body = hyper::body::to_bytes(res.into_body())
        .await
        .expect("failed to read body");
    let body =
        String::from_utf8(body.into_iter().collect()).expect("failed to convert body to string");
    println!("Response body: {}", body);
}
