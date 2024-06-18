#[cfg(all(feature = "poll-io", unix))]
#[monoio::test_all(internal = true)]
async fn test_async_fn() {
    // use libc to create a eventfd
    use libc::{eventfd, EFD_NONBLOCK};
    let eventfd = unsafe { eventfd(0, EFD_NONBLOCK) };
}
