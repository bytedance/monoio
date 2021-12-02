/// You have 3 ways to start the runtime.

fn main() {
    // 1. Create runtime and block_on normally
    let mut rt = monoio::RuntimeBuilder::new().build().unwrap();
    rt.block_on(async {
        println!("it works1!");
    });

    // 2. Create runtime with custom options and block_on
    let mut rt = monoio::RuntimeBuilder::new()
        .with_entries(256)
        .enable_timer()
        .build()
        .unwrap();
    rt.block_on(async {
        println!("it works2!");
    });

    // 3. Use `start` directly: equivalent to default runtime builder and block_on
    monoio::start(async {
        println!("it works3!");
    });
}
