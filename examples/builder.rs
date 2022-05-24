//! Except for using macro, You have 3 ways to start the runtime manually.

fn main() {
    // 1. Create runtime and block_on normally
    let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
        .build()
        .unwrap();
    rt.block_on(async {
        println!("it works1!");
    });

    // 2. Create runtime with custom options and block_on
    let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
        .with_entries(256)
        .enable_timer()
        .build()
        .unwrap();
    rt.block_on(async {
        println!("it works2!");
    });

    // 3. Use `start` directly: equivalent to default runtime builder and block_on
    #[cfg(target_os = "linux")]
    monoio::start::<monoio::IoUringDriver, _>(async {
        println!("it works3!");
    });
    #[cfg(not(target_os = "linux"))]
    monoio::start::<monoio::LegacyDriver, _>(async {
        println!("it works3!");
    });
}
