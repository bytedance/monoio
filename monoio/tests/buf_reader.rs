use futures::FutureExt;
use monoio::{
    io::{AsyncReadRentExt, BufReader},
    net::{TcpListener, TcpStream},
};

#[monoio::test_all(timer_enabled = true)]
async fn buf_reader_use_after_cancel() {
    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap();

    monoio::spawn(async move {
        let (mut stream, _) = srv.accept().await.unwrap();

        // deadlock
        let _ = stream.read_exact(vec![0u8; 1]).await;
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut stream = BufReader::new(stream);

    // Cancel the first read after a timeout
    futures::select_biased! {
        _ = monoio::time::sleep(std::time::Duration::from_millis(50)).fuse() => {},
        _ = stream.read_exact(vec![0u8; 1]).fuse() => unreachable!(),
    }

    // Read again. This should not panic.
    futures::select_biased! {
        _ = monoio::time::sleep(std::time::Duration::from_millis(50)).fuse() => {},
        _ = stream.read_exact(vec![0u8; 1]).fuse() => unreachable!(),
    }
}
