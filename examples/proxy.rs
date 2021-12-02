use monoio::{
    io::{AsyncReadRent, AsyncWriteRentExt},
    net::{TcpListener, TcpStream},
};

const LISTEN_ADDRESS: &str = "127.0.0.1:50005";
const TARGET_ADDRESS: &str = "127.0.0.1:50006";

#[monoio::main(entries = 512, timer_enabled = false)]
async fn main() {
    let listener = TcpListener::bind(LISTEN_ADDRESS)
        .unwrap_or_else(|_| panic!("[Server] Unable to bind to {}", LISTEN_ADDRESS));
    loop {
        if let Ok((in_conn, _addr)) = listener.accept().await {
            let out_conn = TcpStream::connect(TARGET_ADDRESS).await;
            if let Ok(out_conn) = out_conn {
                monoio::spawn(async move {
                    let _ = monoio::join!(
                        copy_one_direction(&in_conn, &out_conn),
                        copy_one_direction(&out_conn, &in_conn),
                    );
                    println!("relay finished");
                });
            } else {
                eprintln!("dial outbound connection failed");
            }
        } else {
            eprintln!("accept connection failed");
            return;
        }
    }
}

async fn copy_one_direction(from: &TcpStream, to: &TcpStream) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = Vec::with_capacity(8 * 1024);
    loop {
        // read
        let (res, _buf) = from.read(buf).await;
        buf = _buf;
        let res: usize = res?;
        if res == 0 {
            return Ok(buf);
        }

        // write all
        let (res, _buf) = to.write_all(buf).await;
        buf = _buf;
        res?;

        // clear
        buf.clear();
    }
}
