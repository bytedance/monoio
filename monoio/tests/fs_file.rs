use std::io::{prelude::*, SeekFrom};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle as RawFd};

use monoio::{
    buf::VecBuf,
    fs::File,
    io::{AsyncReadRent, AsyncWriteRent},
};
use tempfile::NamedTempFile;

const HELLO: &[u8] = b"hello world...";

async fn read_hello(file: &File, offset: u64) {
    let buf = Vec::with_capacity(1024);
    let (res, buf) = file.read_at(buf, offset).await;
    let n = res.unwrap();

    assert!(n <= HELLO.len() - offset as usize);
    assert_eq!(&buf, &HELLO[offset as usize..n + offset as usize]);
}

fn tempfile() -> NamedTempFile {
    NamedTempFile::new().expect("unable to create tempfile")
}

#[monoio::test_all]
async fn basic_read() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();
    tempfile.as_file_mut().sync_data().unwrap();

    let mut file = File::open(tempfile.path()).await.unwrap();

    let (res, buf) = file.read(Vec::with_capacity(HELLO.len() / 2)).await;
    assert!(matches!(res, Ok(len) if len == HELLO.len() / 2));
    assert_eq!(buf, HELLO[..res.unwrap()]);

    let (res, buf) = file.read(Vec::with_capacity(HELLO.len() / 2)).await;
    assert!(matches!(res, Ok(len) if len == HELLO.len() / 2));
    assert_eq!(buf, HELLO[res.unwrap()..]);
}

#[monoio::test_all]
async fn basic_read_vectored() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();
    tempfile.as_file_mut().sync_data().unwrap();

    let mut file = File::open(tempfile.path()).await.unwrap();

    let (res, buf) = file
        .readv(VecBuf::from(vec![vec![0; HELLO.len() / 7]; 2]))
        .await;

    assert!(matches!(res, Ok(len) if len == HELLO.len() / 7 * 2));
    let buf: Vec<_> = Into::<Vec<_>>::into(buf).into_iter().flatten().collect();
    assert_eq!(buf, HELLO[..4]);

    let (res, buf) = file
        .readv(VecBuf::from(vec![vec![0; HELLO.len() / 7]; 5]))
        .await;

    assert!(matches!(res, Ok(len) if len == HELLO.len() / 7 * 5));
    let buf: Vec<_> = Into::<Vec<_>>::into(buf).into_iter().flatten().collect();
    assert_eq!(buf, HELLO[4..]);
}

#[monoio::test_all]
async fn basic_read_at() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();
    tempfile.as_file_mut().sync_data().unwrap();

    let file = File::open(tempfile.path()).await.unwrap();

    for offset in 0..=HELLO.len() {
        read_hello(&file, offset as u64).await;
    }
}

#[monoio::test_all]
async fn basic_read_exact_at() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();
    tempfile.as_file_mut().sync_data().unwrap();

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
async fn read_file_all() {
    use std::io::Write;

    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();
    tempfile.as_file_mut().sync_data().unwrap();

    let res = monoio::fs::read(tempfile.path()).await.unwrap();
    assert_eq!(res, HELLO);
}

#[monoio::test_all]
async fn basic_write() {
    let tempfile = tempfile();
    let mut file = File::create(tempfile.path()).await.unwrap();

    let (res, _) = file.write(HELLO).await;
    assert!(matches!(res, Ok(14)));
    let result = monoio::fs::read(tempfile.path()).await.unwrap();
    assert_eq!(result, HELLO);

    let (res, _) = file.write(HELLO).await;
    assert!(matches!(res, Ok(14)));
    let result = monoio::fs::read(tempfile.path()).await.unwrap();
    assert_eq!(result, [HELLO, HELLO].concat());
}

#[monoio::test_all]
async fn basic_write_vectored() {
    let tempfile = tempfile();
    let mut file = File::create(tempfile.path()).await.unwrap();

    let (res, _) = file.writev(VecBuf::from(vec![HELLO.to_vec(); 2])).await;
    assert!(matches!(res, Ok(len) if len == HELLO.len() * 2));
    let result = monoio::fs::read(tempfile.path()).await.unwrap();
    assert_eq!(result, [HELLO, HELLO].concat());

    let (res, _) = file.writev(VecBuf::from(vec![HELLO.to_vec(); 2])).await;
    assert!(matches!(res, Ok(len) if len == HELLO.len() * 2));
    let result = monoio::fs::read(tempfile.path()).await.unwrap();
    assert_eq!(result, [HELLO, HELLO, HELLO, HELLO].concat());
}

#[monoio::test_all]
async fn basic_write_at() {
    let tempfile = tempfile();

    let file = File::create(tempfile.path()).await.unwrap();
    file.write_at(HELLO, 0).await.0.unwrap();
    file.sync_all().await.unwrap();

    let result = monoio::fs::read(tempfile.path()).await.unwrap();
    assert_eq!(result, HELLO);

    // Modify the file pointer.
    let mut std_file = std::fs::File::open(tempfile.path()).unwrap();
    std_file.seek(SeekFrom::Start(8)).unwrap();

    file.write_at(b"monoio...", 6).await.0.unwrap();
    file.sync_all().await.unwrap();

    assert_eq!(
        monoio::fs::read(tempfile.path()).await.unwrap(),
        b"hello monoio..."
    )
}

#[monoio::test_all]
async fn basic_write_all_at() {
    let tempfile = tempfile();

    let file = File::create(tempfile.path()).await.unwrap();
    file.write_all_at(HELLO, 0).await.0.unwrap();
    file.sync_all().await.unwrap();

    let file = std::fs::read(tempfile.path()).unwrap();
    assert_eq!(file, HELLO);
}

#[monoio::test(driver = "uring")]
async fn cancel_read_at() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();
    tempfile.as_file_mut().sync_data().unwrap();

    let file = File::open(tempfile.path()).await.unwrap();

    // Poll the future once, then cancel it
    poll_once(async { read_hello(&file, 0).await }).await;

    read_hello(&file, 0).await;
}

#[monoio::test_all]
async fn explicit_close() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();
    tempfile.as_file_mut().sync_data().unwrap();

    let file = File::open(tempfile.path()).await.unwrap();
    #[cfg(unix)]
    let fd = file.as_raw_fd();
    #[cfg(windows)]
    let fd = file.as_raw_handle();

    file.close().await.unwrap();

    assert_invalid_fd(fd, tempfile.as_file().metadata().unwrap());
}

#[monoio::test_all]
async fn drop_open() {
    let tempfile = tempfile();

    // Do something else
    let file_w = File::create(tempfile.path()).await.unwrap();
    file_w.write_at(HELLO, 0).await.0.unwrap();
    file_w.sync_all().await.unwrap();

    let file = std::fs::read(tempfile.path()).unwrap();
    assert_eq!(file, HELLO);
    drop(file_w);
}

#[test]
fn drop_off_runtime() {
    let tempfile = tempfile();
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    let file = monoio::start::<monoio::IoUringDriver, _>(async {
        File::open(tempfile.path()).await.unwrap()
    });
    #[cfg(not(all(target_os = "linux", feature = "iouring")))]
    let file = monoio::start::<monoio::LegacyDriver, _>(async {
        File::open(tempfile.path()).await.unwrap()
    });

    #[cfg(unix)]
    let fd = file.as_raw_fd();
    #[cfg(windows)]
    let fd = file.as_raw_handle();
    drop(file);

    assert_invalid_fd(fd, tempfile.as_file().metadata().unwrap());
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

#[allow(unused)]
async fn poll_once(future: impl std::future::Future) {
    use std::{pin::pin, task::Poll};

    use futures::future::poll_fn;

    let mut future = pin!(future);
    poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
}

#[allow(unused, clippy::needless_return)]
fn assert_invalid_fd(fd: RawFd, base: std::fs::Metadata) {
    use std::fs::File;
    #[cfg(unix)]
    let f = unsafe { File::from_raw_fd(fd) };
    #[cfg(windows)]
    let f = unsafe { File::from_raw_handle(fd) };

    let meta = f.metadata();
    std::mem::forget(f);

    if let Ok(meta) = meta {
        if !meta.is_file() {
            return;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let inode = meta.ino();
            let actual = base.ino();
            if inode == actual {
                panic!();
            }
        }
    }
}

#[cfg(unix)]
#[monoio::test_all]
async fn file_from_std() {
    let tempfile = tempfile();
    let std_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(tempfile.path())
        .unwrap();
    let file = File::from_std(std_file).unwrap();
    file.write_at(HELLO, 0).await.0.unwrap();
    file.sync_all().await.unwrap();
    read_hello(&file, 0).await;
}

#[monoio::test_all]
async fn flush_and_shutdown() {
    let tempfile = tempfile();
    let mut file = File::create(tempfile.path()).await.unwrap();

    let res = file.flush().await;
    assert!(matches!(res, Ok(())));

    let res = file.shutdown().await;
    assert!(matches!(res, Ok(())));
}
