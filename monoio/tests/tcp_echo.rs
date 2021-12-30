use monoio::{
    io::{self, AsyncReadRentExt, AsyncWriteRentExt},
    net::{TcpListener, TcpStream},
};

#[monoio::test]
async fn echo_server() {
    const ITER: usize = 1024;

    let (tx, rx) = local_sync::oneshot::channel();

    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap();

    let msg = "foo bar baz";
    monoio::spawn(async move {
        let stream = TcpStream::connect(&addr).await.unwrap();

        for _ in 0..ITER {
            // write
            assert!(stream.write_all(msg).await.0.is_ok());

            // read
            let buf = [0; 11];
            let (res, buf) = stream.read_exact(buf).await;

            assert!(res.is_ok());
            assert_eq!(res.unwrap(), 11);
            assert_eq!(&buf[..], msg.as_bytes());
        }

        assert!(tx.send(()).is_ok());
    });

    let (mut stream, _) = srv.accept().await.unwrap();
    let (rd, wr) = stream.split();

    let n = io::copy(&rd, &wr).await.unwrap();
    assert_eq!(n, (ITER * msg.len()) as u64);

    assert!(rx.await.is_ok());
}
