use monoio::net::unix::UnixDatagram;

#[monoio::test_all]
async fn accept_send_recv() -> std::io::Result<()> {
    let dir = tempfile::Builder::new()
        .prefix("monoio-unix-datagram-tests")
        .tempdir()
        .unwrap();
    let sock_path = dir.path().join("dgram.sock");

    let dgram1 = UnixDatagram::bind(&sock_path)?;
    let dgram2 = UnixDatagram::connect(&sock_path).await?;

    dgram2.send(b"hello").await.0.unwrap();
    let (_res, buf) = dgram1.recv_from(vec![0; 100]).await;
    assert_eq!(buf, b"hello");
    assert!(_res.unwrap().1.is_unnamed());

    let dgram3 = UnixDatagram::unbound()?;
    dgram3.send_to(b"hello2", &sock_path).await.0.unwrap();
    let (res, buf) = dgram1.recv(vec![0; 100]).await;
    assert_eq!(buf, b"hello2");
    assert_eq!(res.unwrap(), 6);
    Ok(())
}

#[monoio::test_all]
async fn addr_type() -> std::io::Result<()> {
    let dir = tempfile::Builder::new()
        .prefix("monoio-unix-datagram-tests")
        .tempdir()
        .unwrap();
    let sock_path1 = dir.path().join("dgram_addr1.sock");
    let sock_path2 = dir.path().join("dgram_addr2.sock");

    let dgram1 = UnixDatagram::bind(&sock_path1)?;
    let dgram2 = UnixDatagram::bind(&sock_path2)?;

    dgram1.send_to(b"hello", sock_path2).await.0.unwrap();
    let (_res, buf) = dgram2.recv_from(vec![0; 100]).await;
    assert_eq!(buf, b"hello");
    assert_eq!(_res.unwrap().1.as_pathname(), Some(sock_path1.as_path()));
    Ok(())
}
