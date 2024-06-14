#[cfg(feature = "signal")]
#[monoio::test(driver = "legacy", internal = true)]
async fn test_ctrlc_legacy() {
    use libc::{getpid, kill, SIGINT};
    use monoio::utils::CtrlC;

    let c = CtrlC::new().unwrap();
    std::thread::spawn(|| unsafe {
        std::thread::sleep(std::time::Duration::from_millis(500));
        kill(getpid(), SIGINT);
    });

    c.await;
}
