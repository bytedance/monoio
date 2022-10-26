//! Example for using h2 directly.
//! Note: This is only meant for compatible usage.
//! Example code is modified from https://github.com/hyperium/h2/blob/master/examples/client.rs.

use monoio::net::TcpStream;
use monoio_compat::StreamWrapper;

#[monoio::main]
async fn main() {
    let tcp = TcpStream::connect("127.0.0.1:5928").await.unwrap();
    let tcp_wrapper = StreamWrapper::new(tcp);
    let (mut client, h2) = h2::client::handshake(tcp_wrapper).await.unwrap();

    println!("sending request");

    let request = http::Request::builder()
        .uri("https://http2.akamai.com/")
        .body(())
        .unwrap();

    let mut trailers = http::HeaderMap::new();
    trailers.insert("zomg", "hello".parse().unwrap());

    let (response, mut stream) = client.send_request(request, false).unwrap();

    // send trailers
    stream.send_trailers(trailers).unwrap();

    // Spawn a task to run the conn...
    monoio::spawn(async move {
        if let Err(e) = h2.await {
            println!("GOT ERR={e:?}");
        }
    });

    let response = response.await.unwrap();
    println!("GOT RESPONSE: {response:?}");

    // Get the body
    let mut body = response.into_body();

    while let Some(chunk) = body.data().await {
        println!("GOT CHUNK = {:?}", chunk.unwrap());
    }

    if let Some(trailers) = body.trailers().await.unwrap() {
        println!("GOT TRAILERS: {trailers:?}");
    }
}
