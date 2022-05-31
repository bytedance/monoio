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
        let mut compat_conn = unsafe { TcpStreamCompat::new(conn) };

        let mut buf = [0u8; 10];
        compat_conn.read_exact(&mut buf).await.unwrap();
        buf[0] += 1;
        compat_conn.write_all(&buf).await.unwrap();
    };
    let client = async {
        oneshot_tx.closed().await;
        let conn = monoio::net::TcpStream::connect(ADDRESS).await.unwrap();
        let mut compat_conn = unsafe { TcpStreamCompat::new(conn) };

        let mut buf = [65u8; 10];
        compat_conn.write_all(&buf).await.unwrap();
        compat_conn.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf[0], 66);
    };
    monoio::join!(client, server);
}
```

Please read the following note before using this crate.

## Important Note
Even with this wrapper, `TcpStreamCompat` is still not fully compatible with the Tokio IO interface.

The user must ensure that once a slice of data is sent using `poll_write` and `poll_read`, it will continue to be used until the call returns `Ready`. Otherwise, the old data will be send.

We implement a simple checking mechanism to ensure that the data is the same between `poll_write`. But for the sake of performance, we only check the data length, which is not enough. It may cause unsoundness.

For example, running `h2` server based on this wrapper will fail. Inside the `h2`, it will try to send a data frame with `poll_write`, and if it get `Pending`, it will assume the data not be sent yet. If there is another data frame with a higher priority, it will `poll_write` the new frame instead. But the old data frame will be sent with our wrapper.

The core problem is caused by incompatible between `poll`-like interface and asynchronous system call.

## TcpStreamCompat and TcpStreamCompatUnsafe
TcpStreamCompat: Will copy data into owned buffer first, then construct save the future. If user does not follow the rule, it will panic.

TcpStreamCompatUnsafe: Will only save user-provided buffer pointer and length. It will not copy the data, so it is more efficient than TcpStreamCompat. But if user does not follow the rule, it will cause memory corruption.
