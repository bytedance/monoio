#![cfg(unix)]

use std::io::Write;

#[monoio::test_all]
async fn basic_file_metadata() {
    let mut file = tempfile::NamedTempFile::new().unwrap();

    assert_eq!(file.write(b"foo bar").unwrap(), 7);

    let m_file = monoio::fs::File::open(file.path()).await.unwrap();

    let m_meta = monoio::fs::metadata(file.path()).await.unwrap();
    let mf_meta = m_file.metadata().await.unwrap();
    let std_meta = std::fs::metadata(file.path()).unwrap();

    assert_eq!(m_meta.len(), std_meta.len());
    assert_eq!(mf_meta.len(), std_meta.len());

    assert_eq!(m_meta.modified().unwrap(), std_meta.modified().unwrap());
    assert_eq!(mf_meta.modified().unwrap(), std_meta.modified().unwrap());

    assert_eq!(m_meta.accessed().unwrap(), std_meta.accessed().unwrap());
    assert_eq!(mf_meta.accessed().unwrap(), std_meta.accessed().unwrap());

    #[cfg(target_os = "linux")]
    assert_eq!(m_meta.created().unwrap(), std_meta.created().unwrap());
    #[cfg(target_os = "linux")]
    assert_eq!(mf_meta.created().unwrap(), std_meta.created().unwrap());

    assert_eq!(m_meta.is_file(), std_meta.is_file());
    assert_eq!(mf_meta.is_file(), std_meta.is_file());

    assert_eq!(m_meta.is_dir(), std_meta.is_dir());
    assert_eq!(mf_meta.is_dir(), std_meta.is_dir());
}

#[monoio::test_all]
async fn dir_metadata() {
    let dir = tempfile::tempdir().unwrap();

    let m_meta = monoio::fs::metadata(dir.path()).await.unwrap();
    let std_meta = std::fs::metadata(dir.path()).unwrap();

    assert_eq!(m_meta.len(), std_meta.len());

    assert_eq!(m_meta.modified().unwrap(), std_meta.modified().unwrap());

    assert_eq!(m_meta.accessed().unwrap(), std_meta.accessed().unwrap());

    #[cfg(target_os = "linux")]
    assert_eq!(m_meta.created().unwrap(), std_meta.created().unwrap());

    assert_eq!(m_meta.is_file(), std_meta.is_file());

    assert_eq!(m_meta.is_dir(), std_meta.is_dir());
}

#[monoio::test_all]
async fn symlink_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(dir.path(), &link).unwrap();

    let m_meta = monoio::fs::symlink_metadata(&link).await.unwrap();
    let std_meta = std::fs::symlink_metadata(&link).unwrap();

    assert_eq!(m_meta.len(), std_meta.len());

    assert_eq!(m_meta.modified().unwrap(), std_meta.modified().unwrap());

    assert_eq!(m_meta.accessed().unwrap(), std_meta.accessed().unwrap());

    #[cfg(target_os = "linux")]
    assert_eq!(m_meta.created().unwrap(), std_meta.created().unwrap());

    assert_eq!(m_meta.is_file(), std_meta.is_file());

    assert_eq!(m_meta.is_dir(), std_meta.is_dir());

    assert_eq!(m_meta.is_symlink(), std_meta.is_symlink());
}
