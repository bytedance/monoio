use std::net::{IpAddr, SocketAddr};

use monoio::net::{TcpListener, TcpStream};

macro_rules! test_connect_ip {
    ($(($ident:ident, $target:expr, $addr_f:path),)*) => {
        $(
            #[monoio::test]
            async fn $ident() {
                let listener = TcpListener::bind($target).unwrap();
                let addr = listener.local_addr().unwrap();
                assert!($addr_f(&addr));

                let (tx, rx) = local_sync::oneshot::channel();

                monoio::spawn(async move {
                    let (socket, addr) = listener.accept().await.unwrap();
                    assert_eq!(addr, socket.peer_addr().unwrap());
                    assert!(tx.send(socket).is_ok());
                });

                let mine = TcpStream::connect(&addr).await.unwrap();
                let theirs = rx.await.unwrap();

                assert_eq!(mine.local_addr().unwrap(), theirs.peer_addr().unwrap());
                assert_eq!(theirs.local_addr().unwrap(), mine.peer_addr().unwrap());
            }
        )*
    }
}

test_connect_ip! {
    (connect_v4, "127.0.0.1:0", SocketAddr::is_ipv4),
    (connect_v6, "[::1]:0", SocketAddr::is_ipv6),
}

macro_rules! test_connect {
    ($(($ident:ident, $mapping:tt),)*) => {
        $(
            #[monoio::test]
            async fn $ident() {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                #[allow(clippy::redundant_closure_call)]
                let addr = $mapping(&listener);

                let server = async {
                    assert!(listener.accept().await.is_ok());
                };

                let client = async {
                    assert!(TcpStream::connect(addr).await.is_ok());
                };

                monoio::join!(server, client);
            }
        )*
    }
}

test_connect! {
    (ip_string, (|listener: &TcpListener| {
        format!("127.0.0.1:{}", listener.local_addr().unwrap().port())
    })),
    (ip_str, (|listener: &TcpListener| {
        let s = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
        let slice: &str = &*Box::leak(s.into_boxed_str());
        slice
    })),
    (ip_port_tuple, (|listener: &TcpListener| {
        let addr = listener.local_addr().unwrap();
        (addr.ip(), addr.port())
    })),
    (ip_port_tuple_ref, (|listener: &TcpListener| {
        let addr = listener.local_addr().unwrap();
        let tuple_ref: &(IpAddr, u16) = &*Box::leak(Box::new((addr.ip(), addr.port())));
        tuple_ref
    })),
    (ip_str_port_tuple, (|listener: &TcpListener| {
        let addr = listener.local_addr().unwrap();
        ("127.0.0.1", addr.port())
    })),
}
