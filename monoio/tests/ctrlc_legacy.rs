#[cfg(feature = "signal")]
#[monoio::test(driver = "legacy")]
async fn test_ctrlc_legacy() {
    use monoio::utils::CtrlC;

    let c = CtrlC::new().unwrap();
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(500));
        CtrlC::ctrlc();
    });

    c.await;
}
