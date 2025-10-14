use std::time::{Duration, Instant};

use futures_lite::future::yield_now;

#[monoio::main(enable_timer = true)]
async fn main() {
    monoio::spawn(async {
        loop {
            let ins = Instant::now();
            yield_now().await;
            println!("task 1: {:?}", ins.elapsed());
        }
    });

    monoio::spawn(async {
        loop {
            let ins = Instant::now();
            yield_now().await;
            println!("task 2: {:?}", ins.elapsed());
        }
    });

    monoio::spawn(async {
        loop {
            let ins = Instant::now();
            monoio::time::sleep(Duration::from_secs(0)).await;
            println!("task 3: {:?} ⭐️", ins.elapsed());
        }
    });

    std::future::pending().await
}
