# Monoio Compat

A compat wrapper for monoio.

Usage example:
```
use monoio_compat::{AsyncReadExt, AsyncWriteExt, TcpStreamCompat};

#[monoio::main]
async fn main() {
    const ADDRESS: &str = "127.0.0.1:50009";
    let (mut oneshot_tx, mut oneshot_rx) = local_sync::oneshot::channel::<()>();

    let server = async move {
        let listener = monoio::net::TcpListener::bind(ADDRESS).unwrap();
        oneshot_rx.close();
        let (conn, _) = listener.accept().await.unwrap();
        let mut compat_conn: TcpStreamCompat = conn.into();

        let mut buf = [0u8; 10];
        compat_conn.read_exact(&mut buf).await.unwrap();
        buf[0] += 1;
        compat_conn.write_all(&buf).await.unwrap();
    };
    let client = async {
        oneshot_tx.closed().await;
        let conn = monoio::net::TcpStream::connect(ADDRESS).await.unwrap();
        let mut compat_conn: TcpStreamCompat = conn.into();

        let mut buf = [65u8; 10];
        compat_conn.write_all(&buf).await.unwrap();
        compat_conn.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf[0], 66);
    };
    monoio::join!(client, server);
}
```
