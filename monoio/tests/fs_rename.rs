#![cfg(feature = "renameat")]

#[monoio::test_all]
async fn rename_file_in_the_same_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = tempfile::NamedTempFile::new_in(temp_dir.path()).unwrap();

    let old_file_path = file.path();
    let new_file_path = temp_dir.path().join("test-file");

    let result = monoio::fs::rename(old_file_path, &new_file_path).await;
    assert!(result.is_ok());

    assert!(new_file_path.exists());
    assert!(!old_file_path.exists());
}

#[monoio::test_all]
async fn rename_file_in_different_directory() {
    let temp_dir1 = tempfile::tempdir().unwrap();
    let temp_dir2 = tempfile::tempdir().unwrap();
    let file = tempfile::NamedTempFile::new_in(temp_dir1.path()).unwrap();

    let old_file_path = file.path();
    let new_file_path = temp_dir2.path().join("test-file");

    let result = monoio::fs::rename(old_file_path, &new_file_path).await;
    assert!(result.is_ok());

    assert!(new_file_path.exists());
    assert!(!old_file_path.exists());
}

#[monoio::test_all]
async fn mv_file_in_different_directory() {
    let temp_dir1 = tempfile::tempdir().unwrap();
    let temp_dir2 = tempfile::tempdir().unwrap();
    let file = tempfile::NamedTempFile::new_in(temp_dir1.path()).unwrap();

    let old_file_path = file.path();
    let old_file_name = old_file_path.file_name().unwrap();
    let new_file_path = temp_dir2.path().join(old_file_name);

    let result = monoio::fs::rename(old_file_path, &new_file_path).await;
    assert!(result.is_ok());

    assert!(new_file_path.exists());
    assert!(!old_file_path.exists());
}

#[monoio::test_all]
async fn rename_nonexistent_file() {
    let temp_dir = tempfile::tempdir().unwrap();

    let old_file_path = temp_dir.path().join("nonexistent.txt");
    let new_file_path = temp_dir.path().join("renamed.txt");

    let result = monoio::fs::rename(old_file_path, new_file_path).await;

    assert!(result.is_err());
}

#[cfg(unix)]
#[monoio::test_all]
async fn rename_file_without_permission() {
    use std::{fs::Permissions, os::unix::fs::PermissionsExt};

    let temp_dir = tempfile::tempdir().unwrap();
    let temp_file = tempfile::NamedTempFile::new_in(&temp_dir).unwrap();

    std::fs::set_permissions(temp_dir.path(), Permissions::from_mode(0o0)).unwrap();

    let old_file_path = temp_file.path();
    let new_file_path = temp_dir.path().join("test-file");

    let result = monoio::fs::rename(old_file_path, &new_file_path).await;

    assert!(result.is_err());
}
