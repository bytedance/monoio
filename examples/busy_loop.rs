use std::{
    future::Future,
    pin::Pin,
    task::{
        Context,
        Poll::{self, Ready},
    },
    time::{Duration, Instant},
};

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

pub fn yield_now() -> YieldNow {
    YieldNow { yielded: false }
}

pub struct YieldNow {
    yielded: bool,
}

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
