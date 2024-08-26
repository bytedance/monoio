use tempfile::NamedTempFile;

const HELLO: &[u8] = b"hello world...";

fn tempfile() -> NamedTempFile {
    NamedTempFile::new().expect("unable to create tempfile")
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
