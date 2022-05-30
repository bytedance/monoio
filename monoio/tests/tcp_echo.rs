use monoio::{
    io::{self, AsyncReadRentExt, AsyncWriteRentExt},
    net::{TcpListener, TcpStream},
};

#[monoio::test_all]
async fn echo_server() {
    const ITER: usize = 1024;

    let (tx, rx) = local_sync::oneshot::channel();

    let srv = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = srv.local_addr().unwrap();

    let msg = "foo bar baz";
    let iov_msg = "iovec_is_so_good";
    monoio::spawn(async move {
        let mut stream = TcpStream::connect(&addr).await.unwrap();

        let mut buf_vec_to_write: Option<Vec<Vec<u8>>> = Some(vec![
            iov_msg.as_bytes()[..9].into(),
            iov_msg.as_bytes()[9..].into(),
        ]);
        for _ in 0..ITER {
            // write
            assert!(stream.write_all(msg).await.0.is_ok());

            // read
            let buf = [0; 11];
            let (res, buf) = stream.read_exact(buf).await;
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), 11);
            assert_eq!(&buf[..], msg.as_bytes());

            // writev
            let buf_vec: monoio::buf::VecBuf = buf_vec_to_write.take().unwrap().into();
            let (res, buf_vec) = stream.write_vectored_all(buf_vec).await;
            let raw_vec: Vec<Vec<u8>> = buf_vec.into();
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), iov_msg.len());
            buf_vec_to_write = Some(raw_vec);

            // readv
            let buf_vec: monoio::buf::VecBuf = vec![vec![0; 3], vec![0; iov_msg.len() - 3]].into();
            let (res, buf_vec) = stream.read_vectored_exact(buf_vec).await;
            assert!(res.is_ok());
            assert_eq!(res.unwrap(), iov_msg.len());
            let raw_vec: Vec<Vec<u8>> = buf_vec.into();
            assert_eq!(&raw_vec[0], &iov_msg.as_bytes()[..3]);
            assert_eq!(&raw_vec[1], &iov_msg.as_bytes()[3..]);
        }

        assert!(tx.send(()).is_ok());
    });

    let (mut stream, _) = srv.accept().await.unwrap();
    let (mut rd, mut wr) = stream.split();

    let n = io::copy(&mut rd, &mut wr).await.unwrap();
    assert_eq!(n, (ITER * (msg.len() + 16)) as u64);

    assert!(rx.await.is_ok());
}
