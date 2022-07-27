//! You may not call thread::sleep in async runtime, which will block the whole
//! thread. Instead, you should use monoio::time provided functions.

use std::time::Duration;

#[monoio::main(enable_timer = true)]
async fn main() {
    loop {
        monoio::time::sleep(Duration::from_secs(1)).await;
        println!("balabala");
        monoio::time::sleep(Duration::from_secs(1)).await;
        println!("abaaba");
    }
}
