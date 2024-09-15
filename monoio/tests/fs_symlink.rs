#![cfg(all(unix, feature = "symlinkat"))]

use std::{io, path::PathBuf};

use monoio::fs::File;
use tempfile::tempdir;

const TEST_PAYLOAD: &[u8] = b"I am data in the source file";

async fn create_file(path: &PathBuf) -> io::Result<File> {
    File::create(path).await
}

#[monoio::test_all]
async fn create_symlink() {
    let tmpdir = tempdir().unwrap();
    let src_file_path = tmpdir.path().join("src");
    let dst_file_path = tmpdir.path().join("dst");
    let src_file = create_file(&src_file_path).await.unwrap();
    src_file.write_all_at(TEST_PAYLOAD, 0).await.0.unwrap();
    monoio::fs::symlink(src_file_path.as_path(), dst_file_path.as_path())
        .await
        .unwrap();

    let content = monoio::fs::read(&dst_file_path).await.unwrap();
    assert_eq!(content, TEST_PAYLOAD);
    assert!(dst_file_path.is_symlink());
}
