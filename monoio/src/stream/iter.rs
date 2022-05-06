use super::{assert_stream, Stream};
use std::future::Future;

/// Stream for the [`iter`] function.
#[derive(Debug, Clone)]
#[must_use = "streams do nothing unless polled"]
pub struct Iter<I> {
    iter: I,
}

/// Converts an `Iterator` into a `Stream` which is always ready
/// to yield the next value.
pub fn iter<I>(i: I) -> Iter<I::IntoIter>
where
    I: IntoIterator,
{
    assert_stream::<I::Item, _>(Iter {
        iter: i.into_iter(),
    })
}

impl<I> Stream for Iter<I>
where
    I: Iterator,
{
    type Item = I::Item;

    type Future<'a> = impl Future<Output = Option<Self::Item>> where
        I: 'a;

    fn next(&mut self) -> Self::Future<'_> {
        async move { self.iter.next() }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
