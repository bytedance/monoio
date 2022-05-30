use monoio::fs::File;
use std::io::prelude::*;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use tempfile::NamedTempFile;

const HELLO: &[u8] = b"hello world...";

async fn read_hello(file: &File) {
    let buf = Vec::with_capacity(1024);
    let (res, buf) = file.read_at(buf, 0).await;
    let n = res.unwrap();

    assert!(n > 0 && n <= HELLO.len());
    assert_eq!(&buf, &HELLO[..n]);
}

#[monoio::test_all]
async fn basic_read() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();

    let file = File::open(tempfile.path()).await.unwrap();
    read_hello(&file).await;
}

#[monoio::test_all]
async fn basic_read_exact() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();

    let file = File::open(tempfile.path()).await.unwrap();
    let buf = Vec::with_capacity(HELLO.len());
    let (res, buf) = file.read_exact_at(buf, 0).await;
    res.unwrap();
    assert_eq!(&buf[..], HELLO);

    let buf = Vec::with_capacity(HELLO.len() * 2);
    let (res, _) = file.read_exact_at(buf, 0).await;
    assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::UnexpectedEof);
}

#[monoio::test_all]
async fn basic_write() {
    let tempfile = tempfile();

    let file = File::create(tempfile.path()).await.unwrap();
    file.write_at(HELLO, 0).await.0.unwrap();

    let file = std::fs::read(tempfile.path()).unwrap();
    assert_eq!(file, HELLO);
}

#[monoio::test_all]
async fn basic_write_all() {
    let tempfile = tempfile();

    let file = File::create(tempfile.path()).await.unwrap();
    file.write_all_at(HELLO, 0).await.0.unwrap();

    let file = std::fs::read(tempfile.path()).unwrap();
    assert_eq!(file, HELLO);
}

#[monoio::test(driver = "uring")]
async fn cancel_read() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();

    let file = File::open(tempfile.path()).await.unwrap();

    // Poll the future once, then cancel it
    poll_once(async { read_hello(&file).await }).await;

    read_hello(&file).await;
}

#[monoio::test_all]
async fn explicit_close() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();

    let file = File::open(tempfile.path()).await.unwrap();
    let fd = file.as_raw_fd();

    file.close().await.unwrap();

    assert_invalid_fd(fd);
}

#[monoio::test_all]
async fn drop_open() {
    let tempfile = tempfile();
    let _ = File::create(tempfile.path());

    // Do something else
    let file = File::create(tempfile.path()).await.unwrap();

    file.write_at(HELLO, 0).await.0.unwrap();
    drop(file);

    let file = std::fs::read(tempfile.path()).unwrap();
    assert_eq!(file, HELLO);
}

#[test]
fn drop_off_runtime() {
    #[cfg(target_os = "linux")]
    let file = monoio::start::<monoio::IoUringDriver, _>(async {
        let tempfile = tempfile();
        File::open(tempfile.path()).await.unwrap()
    });
    #[cfg(not(target_os = "linux"))]
    let file = monoio::start::<monoio::LegacyDriver, _>(async {
        let tempfile = tempfile();
        File::open(tempfile.path()).await.unwrap()
    });

    let fd = file.as_raw_fd();
    drop(file);

    assert_invalid_fd(fd);
}

#[monoio::test_all]
async fn sync_doesnt_kill_anything() {
    let tempfile = tempfile();

    let file = File::create(tempfile.path()).await.unwrap();
    file.sync_all().await.unwrap();
    file.sync_data().await.unwrap();
    file.write_at(&b"foo"[..], 0).await.0.unwrap();
    file.sync_all().await.unwrap();
    file.sync_data().await.unwrap();
}

fn tempfile() -> NamedTempFile {
    NamedTempFile::new().expect("unable to create tempfile")
}

#[allow(unused)]
async fn poll_once(future: impl std::future::Future) {
    use futures::future::poll_fn;
    use monoio::pin;
    use std::task::Poll;

    pin!(future);

    poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
}

fn assert_invalid_fd(fd: RawFd) {
    use std::fs::File;

    let mut f = unsafe { File::from_raw_fd(fd) };
    let mut buf = vec![];

    assert!(f.read_to_end(&mut buf).is_err());
}
