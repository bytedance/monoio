/// You may not call thread::sleep in async runtime, which will block the whole thread.
/// Instead, you should use monoio::time provided functions.
use monoio::time::Duration;

fn main() {
    let mut rt = monoio::RuntimeBuilder::new()
        .enable_timer()
        .build()
        .unwrap();
    rt.block_on(wait_print());
}

async fn wait_print() {
    loop {
        monoio::time::sleep(Duration::from_secs(1)).await;
        println!("balabala");
        monoio::time::sleep(Duration::from_secs(1)).await;
        println!("abaaba");
    }
}
