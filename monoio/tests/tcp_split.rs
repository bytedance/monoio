use std::{
    io::{Read, Result, Write},
    thread,
};

use monoio::{
    io::{AsyncReadRent, AsyncWriteRentExt},
    net::TcpStream,
};

#[monoio::test_all]
async fn split() -> Result<()> {
    const MSG: &[u8] = b"split";

    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.write_all(MSG).unwrap();

        let mut read_buf = [0u8; 32];
        let read_len = stream.read(&mut read_buf).unwrap();
        assert_eq!(&read_buf[..read_len], MSG);
    });

    let mut stream = TcpStream::connect(&addr).await?;
    let (mut read_half, mut write_half) = stream.split();

    let read_buf = [0u8; 32];
    let (read_res, buf) = read_half.read(read_buf).await;
    assert_eq!(read_res.unwrap(), MSG.len());
    assert_eq!(&buf[..MSG.len()], MSG);

    write_half.write_all(MSG).await.0?;
    handle.join().unwrap();
    Ok(())
}
