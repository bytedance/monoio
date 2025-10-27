use std::{
    pin::Pin,
    task::{Context, Poll},
};

use monoio::io::stream::Stream;

#[must_use = "streams do nothing unless polled"]
pub struct FuturesCompat<St> {
    stream: St,
}

impl<St> FuturesCompat<St>
where
    St: Stream,
{
    pub fn new(stream: St) -> Self {
        Self { stream }
    }
}

impl<St> futures::Stream for FuturesCompat<St>
where
    St: Stream + Unpin,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let future = self.get_mut().stream.next();
        pin_mut!(future);
        future.poll(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
