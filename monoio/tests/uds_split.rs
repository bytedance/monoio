use monoio::{
    io::{AsyncReadRent, AsyncReadRentExt, AsyncWriteRent, AsyncWriteRentExt},
    net::UnixStream,
};

/// Checks that `UnixStream` can be split into a read half and a write half using
/// `UnixStream::split` and `UnixStream::split_mut`.
///
/// Verifies that the implementation of `AsyncWrite::poll_shutdown` shutdowns the stream for
/// writing by reading to the end of stream on the other side of the connection.
#[monoio::test_all(entries = 1024)]
async fn split() -> std::io::Result<()> {
    let (mut a, mut b) = UnixStream::pair()?;

    let (mut a_read, mut a_write) = a.split();
    let (mut b_read, mut b_write) = b.split();

    let (a_response, b_response) = futures::future::try_join(
        send_recv_all(&mut a_read, &mut a_write, b"A"),
        send_recv_all(&mut b_read, &mut b_write, b"B"),
    )
    .await?;

    assert_eq!(a_response, b"B");
    assert_eq!(b_response, b"A");

    Ok(())
}

async fn send_recv_all<R: AsyncReadRent, W: AsyncWriteRent>(
    read: &mut R,
    write: &mut W,
    input: &'static [u8],
) -> std::io::Result<Vec<u8>> {
    write.write_all(input).await.0?;
    write.shutdown().await?;

    let output = Vec::with_capacity(2);
    let (res, buf) = read.read_exact(output).await;
    assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::UnexpectedEof);
    Ok(buf)
}
