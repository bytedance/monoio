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

    #[cfg(target_os = "linux")]
    assert!(_res.unwrap().1.is_unnamed());

    let dgram3 = UnixDatagram::unbound()?;
    dgram3.send_to(b"hello2", &sock_path).await.0.unwrap();
    let (res, buf) = dgram1.recv(vec![0; 100]).await;
    assert_eq!(buf, b"hello2");
    assert_eq!(res.unwrap(), 6);
    Ok(())
}
