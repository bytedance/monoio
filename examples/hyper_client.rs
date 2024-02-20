//! HTTP client example with hyper in poll-io mode.
//!
//! It will try to fetch http://httpbin.org/ip and print the
//! response.
#[cfg(unix)]
use {
    bytes::Bytes,
    http_body_util::{BodyExt, Empty},
    hyper::Request,
    monoio::{io::IntoPollIo, net::TcpStream},
    monoio_compat::hyper::MonoioIo,
    std::io::Write,
};

#[cfg(unix)]
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[cfg(unix)]
async fn fetch_url(url: hyper::Uri) -> Result<()> {
    let host = url.host().expect("uri has no host");
    let port = url.port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(addr).await?.into_poll_io()?;
    let io = MonoioIo::new(stream);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    monoio::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    let authority = url.authority().unwrap().clone();

    let path = url.path();
    let req = Request::builder()
        .uri(path)
        .header(hyper::header::HOST, authority.as_str())
        .body(Empty::<Bytes>::new())?;

    let mut res = sender.send_request(req).await?;

    println!("Response: {}", res.status());
    println!("Headers: {:#?}\n", res.headers());

    // Stream the body, writing each chunk to stdout as we get it
    // (instead of buffering and printing at the end).
    while let Some(next) = res.frame().await {
        let frame = next?;
        if let Some(chunk) = frame.data_ref() {
            std::io::stdout().write_all(chunk)?;
        }
    }
    println!("\n\nDone!");

    Ok(())
}

#[monoio::main]
async fn main() {
    let url = "http://httpbin.org/ip".parse::<hyper::Uri>().unwrap();
    fetch_url(url).await.unwrap();
}
