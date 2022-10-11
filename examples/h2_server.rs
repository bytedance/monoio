//! Example for using h2 directly.
//! Note: This is only meant for compatible usage.
//! Example code is modified from https://github.com/hyperium/h2/blob/master/examples/server.rs.

use monoio::net::{TcpListener, TcpStream};
use monoio_compat::StreamWrapper;

async fn serve(socket: TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let socket_wrapper = StreamWrapper::new(socket);
    let mut connection = h2::server::handshake(socket_wrapper).await?;
    println!("H2 connection bound");

    while let Some(result) = connection.accept().await {
        let (request, respond) = result?;
        monoio::spawn(async move {
            if let Err(e) = handle_request(request, respond).await {
                println!("error while handling request: {}", e);
            }
        });
    }

    println!("~~~~~~~~~~~ H2 connection CLOSE !!!!!! ~~~~~~~~~~~");
    Ok(())
}

async fn handle_request(
    mut request: http::Request<h2::RecvStream>,
    mut respond: h2::server::SendResponse<bytes::Bytes>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("GOT request: {:?}", request);

    let body = request.body_mut();
    while let Some(data) = body.data().await {
        let data = data?;
        println!("<<<< recv {:?}", data);
        let _ = body.flow_control().release_capacity(data.len());
    }

    let response = http::Response::new(());
    let mut send = respond.send_response(response, false)?;
    println!(">>>> send");
    send.send_data(bytes::Bytes::from_static(b"hello "), false)?;
    send.send_data(bytes::Bytes::from_static(b"world\n"), true)?;

    Ok(())
}

#[monoio::main(threads = 2)]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:5928").unwrap();
    println!("listening on {:?}", listener.local_addr());

    loop {
        if let Ok((socket, _peer_addr)) = listener.accept().await {
            monoio::spawn(async move {
                if let Err(e) = serve(socket).await {
                    println!("  -> err={:?}", e);
                }
            });
        }
    }
}
