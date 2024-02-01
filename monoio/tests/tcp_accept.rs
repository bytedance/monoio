macro_rules! test_accept {
    ($(($ident:ident, $target:expr),)*) => {
        $(
            // will report { code: 38, kind: Unsupported, message: "Function not implemented" } in aarch64,
            // armv7, riscv64gc, s390x, just ignore
            #[cfg(not(any(
                target_arch = "aarch64",
                target_arch = "arm",
                target_arch = "riscv64",
                target_arch = "s390x",
            )))]
            #[monoio::test_all]
            async fn $ident() {
                use std::net::{IpAddr, SocketAddr};
                use monoio::net::{TcpListener, TcpStream};

                let listener = TcpListener::bind($target).unwrap();
                let addr = listener.local_addr().unwrap();
                let (tx, rx) = local_sync::oneshot::channel();
                monoio::spawn(async move {
                    let (socket, _) = listener.accept().await.unwrap();
                    assert!(tx.send(socket).is_ok());
                });
                let cli = TcpStream::connect(&addr).await.unwrap();
                let srv = rx.await.unwrap();
                assert_eq!(cli.local_addr().unwrap(), srv.peer_addr().unwrap());
            }
        )*
    }
}

test_accept! {
    (ip_str, "127.0.0.1:0"),
    (host_str, "localhost:0"),
    (socket_addr, "127.0.0.1:0".parse::<SocketAddr>().unwrap()),
    (str_port_tuple, ("127.0.0.1", 0)),
    (ip_port_tuple, ("127.0.0.1".parse::<IpAddr>().unwrap(), 0)),
}
