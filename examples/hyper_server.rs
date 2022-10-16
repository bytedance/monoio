//! HTTP server example with hyper in compatible mode.
//!
//! After running this example, you can open http://localhost:23300
//! and http://localhost:23300/monoio in your browser or curl it.
//! Also you can try `hyper_client.rs` example to request it.

use std::net::SocketAddr;

use futures::Future;
use hyper::{server::conn::Http, service::service_fn};
use monoio::net::TcpListener;
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

pub(crate) async fn serve_http<S, F, R, A>(addr: A, service: S) -> std::io::Result<()>
where
    S: FnMut(Request<Body>) -> F + 'static + Copy,
    F: Future<Output = Result<Response<Body>, R>> + 'static,
    R: std::error::Error + 'static + Send + Sync,
    A: Into<SocketAddr>,
{
    let listener = TcpListener::bind(addr.into())?;
    loop {
        let (stream, _) = listener.accept().await?;
        monoio::spawn(
            Http::new()
                .with_executor(HyperExecutor)
                .serve_connection(TcpStreamCompat::new(stream), service_fn(service)),
        );
    }
}

use hyper::{Body, Method, Request, Response, StatusCode};

async fn hyper_handler(req: Request<Body>) -> Result<Response<Body>, std::convert::Infallible> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => Ok(Response::new(Body::from("Hello World!"))),
        (&Method::GET, "/monoio") => Ok(Response::new(Body::from("Hello Monoio!"))),
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 not found"))
            .unwrap()),
    }
}

#[monoio::main(threads = 2)]
async fn main() {
    println!("Running http server on 0.0.0.0:23300");
    let _ = serve_http(([0, 0, 0, 0], 23300), hyper_handler).await;
    println!("Http server stopped");
}
