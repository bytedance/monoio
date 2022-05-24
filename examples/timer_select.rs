//! An example to illustrate selecting by macro or manually.

use monoio::time::Duration;
use pin_project_lite::pin_project;
use std::future::Future;

#[monoio::main(enable_timer = true)]
async fn main() {
    loop {
        // You can write poll by yourself
        let fut1 = monoio::time::sleep(Duration::from_secs(1));
        let fut2 = monoio::time::sleep(Duration::from_secs(20));
        let dual_select = DualSelect { fut1, fut2 };

        dual_select.await;
        println!("balabala");

        // Or you can use select macro
        let fut1 = monoio::time::sleep(Duration::from_secs(1));
        let fut2 = monoio::time::sleep(Duration::from_secs(20));
        monoio::select! {
            _ = fut1 => {
                println!("balabala in select macro");
            }
            _ = fut2 => {
                println!("abaaba");
            }
        }
    }
}

pin_project! {
    struct DualSelect<F> {
        #[pin]
        fut1: F,
        #[pin]
        fut2: F
    }
}

impl<F> Future for DualSelect<F>
where
    F: Future<Output = ()>,
{
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();

        match this.fut1.poll(cx) {
            std::task::Poll::Ready(_) => return std::task::Poll::Ready(()),
            std::task::Poll::Pending => {}
        }
        match this.fut2.poll(cx) {
            std::task::Poll::Ready(_) => std::task::Poll::Ready(()),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
