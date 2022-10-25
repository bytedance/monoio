//! You can use async channel between threads(with `sync` feature).
//! Remember: it is not efficient. You should rely thread local for hot paths.

use std::time::Duration;

use futures::channel::oneshot;

#[monoio::main]
async fn main() {
    let (tx, rx) = oneshot::channel::<u8>();
    let t = std::thread::spawn(move || {
        println!("remote thread created");
        let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
            .build()
            .unwrap();
        rt.block_on(async move {
            let n = rx.await;
            println!("await result: {n:?}");
        });
        println!("remote thread exit");
    });

    std::thread::sleep(Duration::from_secs(1));
    println!("send local: {:?}", tx.send(1));
    println!("wait for remote thread");
    let _ = t.join();
}
