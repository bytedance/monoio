#![cfg(feature = "sync")]
use std::io::prelude::*;

use monoio::{
    blocking::DefaultThreadPool, buf::VecBuf, fs::File, io::AsyncReadRent, LegacyDriver, Runtime,
    RuntimeBuilder,
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

fn create_runtime() -> Runtime<LegacyDriver> {
    let builder = RuntimeBuilder::<LegacyDriver>::new()
        .attach_thread_pool(Box::new(DefaultThreadPool::new(4)));
    builder.build().unwrap()
}

#[test]
fn basic_non_blocking_read() {
    create_runtime().block_on(async {
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
    })
}

#[test]
fn basic_non_blocking_read_vectored() {
    create_runtime().block_on(async {
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
    })
}

#[test]
fn basic_non_blocking_read_at() {
    create_runtime().block_on(async {
        let mut tempfile = tempfile();
        tempfile.write_all(HELLO).unwrap();
        tempfile.as_file_mut().sync_data().unwrap();

        let file = File::open(tempfile.path()).await.unwrap();

        for offset in 0..=HELLO.len() {
            read_hello(&file, offset as u64).await;
        }
    })
}

#[test]
fn non_blocking_read_file_all() {
    create_runtime().block_on(async {
        use std::io::Write;

        let mut tempfile = tempfile();
        tempfile.write_all(HELLO).unwrap();
        tempfile.as_file_mut().sync_data().unwrap();

        let res = monoio::fs::read(tempfile.path()).await.unwrap();
        assert_eq!(res, HELLO);
    })
}
