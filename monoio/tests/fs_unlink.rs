#![cfg(all(feature = "unlinkat", feature = "mkdirat"))]

use std::{io, path::PathBuf};

use monoio::fs::{self, File};
use tempfile::tempdir;

async fn create_file(path: &PathBuf) -> io::Result<()> {
    let file = File::create(path).await?;
    file.close().await?;
    Ok(())
}

#[monoio::test_all]
async fn remove_file() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("test");

    create_file(&target).await.unwrap();
    fs::remove_file(&target).await.unwrap();
    assert!(File::open(&target).await.is_err());
    assert!(fs::remove_file(&target).await.is_err());
}

#[monoio::test_all]
async fn remove_dir() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("test");

    fs::create_dir(&target).await.unwrap();
    let path = target.join("file");
    create_file(&path).await.unwrap();
    assert!(fs::remove_dir(&target).await.is_err()); // dir is not empty
    fs::remove_file(&path).await.unwrap();
    fs::remove_dir(&target).await.unwrap();
    assert!(create_file(&path).await.is_err()); // dir has been removed
    assert!(fs::remove_dir(&target).await.is_err());
}
