use std::future::Future;

use super::{assert_stream, Stream};

/// Stream extensions.
pub trait StreamExt: Stream {
    /// Maps a stream to a stream of its items.
    fn map<T, F>(self, f: F) -> Map<Self, F>
    where
        F: FnMut(Self::Item) -> T,
        Self: Sized,
    {
        assert_stream::<T, _>(Map::new(self, f))
    }

    /// Computes from this stream's items new items of a different type using
    /// an asynchronous closure.
    fn then<Fut, F>(self, f: F) -> Then<Self, F>
    where
        F: FnMut(Self::Item) -> Fut,
        Fut: Future,
        Self: Sized,
    {
        assert_stream::<Fut::Output, _>(Then::new(self, f))
    }

    /// Runs this stream to completion, executing the provided asynchronous
    /// closure for each element on the stream.
    fn for_each<Fut, F>(mut self, mut f: F) -> ForEachFut<Self, Fut, F>
    where
        F: FnMut(Self::Item) -> Fut,
        Fut: Future<Output = ()>,
        Self: Sized,
    {
        async move {
            while let Some(item) = self.next().await {
                (f)(item).await;
            }
        }
    }
}

impl<T> StreamExt for T where T: Stream {}

type ForEachFut<T: Stream, Fut: Future<Output = ()>, F: FnMut(<T as Stream>::Item) -> Fut> =
    impl Future<Output = ()>;

#[must_use = "streams do nothing unless polled"]
pub struct Map<St, F> {
    stream: St,
    f: F,
}

impl<St, F> Map<St, F> {
    pub(crate) fn new(stream: St, f: F) -> Self {
        Self { stream, f }
    }
}

impl<St, F, Item> Stream for Map<St, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Item,
{
    type Item = Item;

    type NextFuture<'a> = impl Future<Output = Option<Self::Item>> + 'a where
        F: 'a, St: 'a;

    fn next(&mut self) -> Self::NextFuture<'_> {
        async move { self.stream.next().await.map(&mut self.f) }
    }
}

#[must_use = "streams do nothing unless polled"]
pub struct Then<St, F> {
    stream: St,
    f: F,
}

impl<St, F> Then<St, F>
where
    St: Stream,
{
    pub(super) fn new(stream: St, f: F) -> Self {
        Self { stream, f }
    }
}

impl<St, Fut, F> Stream for Then<St, F>
where
    St: Stream,
    F: FnMut(St::Item) -> Fut,
    Fut: Future,
{
    type Item = Fut::Output;

    type NextFuture<'a> = impl Future<Output = Option<Self::Item>> + 'a where
        F: 'a, St: 'a,;

    fn next(&mut self) -> Self::NextFuture<'_> {
        async move {
            let item = self.stream.next().await?;
            Some((self.f)(item).await)
        }
    }
}
