use monoio::{buf::VecBuf, io::AsyncReadRent};

#[monoio::test_all]
async fn test_async_read_rent_for_mut_slice() {
    let mut src: &mut [u8] = &mut *Box::new(*b"hello world");
    let dst = vec![0u8; 5];

    // Read 5 bytes from src into dst
    let (res, dst) = src.read(dst).await;
    assert_eq!(res.unwrap(), 5);
    assert_eq!(&dst, b"hello");

    // Read the rest
    let dst2 = vec![0u8; 6];
    let (res, dst2) = src.read(dst2).await;
    assert_eq!(res.unwrap(), 6);
    assert_eq!(&dst2, b" world");

    // Now src should be empty
    let dst3 = vec![0u8; 1];
    let (res, _) = src.read(dst3).await;
    assert_eq!(res.unwrap(), 0);
}

#[monoio::test_all]
async fn test_async_read_rent_for_mut_slice_readv() {
    let mut src: &mut [u8] = &mut *Box::new(*b"hello world");
    let buf_vec = VecBuf::from(vec![vec![0u8; 5], vec![0u8; 6]]);
    let (res, buf_vec) = src.readv(buf_vec).await;
    assert_eq!(res.unwrap(), 11);
    let raw_vec: Vec<Vec<u8>> = buf_vec.into();
    assert_eq!(&raw_vec[0], b"hello");
    assert_eq!(&raw_vec[1], b" world");
    // Now src should be empty
    let buf_vec = VecBuf::from(vec![vec![0u8; 1]]);
    let (res, _) = src.readv(buf_vec).await;
    assert_eq!(res.unwrap(), 0);
}

#[monoio::test_all]
async fn test_mutability_after_read() {
    let mut backing = *b"hello world";
    let mut src: &mut [u8] = &mut backing;
    let dst = vec![0u8; 5];
    let (_res, _dst) = src.read(dst).await;
    // Mutate the remaining part of the slice
    if !src.is_empty() {
        src[0] = b'X';
    }
    // The original buffer should now be b"helloXworld"
    assert_eq!(&backing, b"helloXworld");
}
