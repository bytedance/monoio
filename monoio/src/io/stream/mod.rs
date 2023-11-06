//! Stream trait in GAT style.

mod iter;
mod stream_ext;

use std::future::Future;

pub use iter::{iter, Iter};
pub use stream_ext::StreamExt;

/// A stream of values produced asynchronously in pure async/await.
#[must_use = "streams do nothing unless polled"]
pub trait Stream {
    /// Values yielded by the stream.
    type Item;

    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    fn next(&mut self) -> impl Future<Output = Option<Self::Item>>;

    /// Returns the bounds on the remaining length of the stream.
    ///
    /// Specifically, `size_hint()` returns a tuple where the first element
    /// is the lower bound, and the second element is the upper bound.
    ///
    /// The second half of the tuple that is returned is an
    /// [`Option`]`<`[`usize`]`>`. A [`None`] here means that either there
    /// is no known upper bound, or the upper bound is larger than
    /// [`usize`].
    ///
    /// # Implementation notes
    ///
    /// It is not enforced that a stream implementation yields the declared
    /// number of elements. A buggy stream may yield less than the lower bound
    /// or more than the upper bound of elements.
    ///
    /// `size_hint()` is primarily intended to be used for optimizations such as
    /// reserving space for the elements of the stream, but must not be
    /// trusted to e.g., omit bounds checks in unsafe code. An incorrect
    /// implementation of `size_hint()` should not lead to memory safety
    /// violations.
    ///
    /// That said, the implementation should provide a correct estimation,
    /// because otherwise it would be a violation of the trait's protocol.
    ///
    /// The default implementation returns `(0, `[`None`]`)` which is correct
    /// for any stream.
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<S: ?Sized + Stream> Stream for &mut S {
    type Item = S::Item;

    fn next(&mut self) -> impl Future<Output = Option<Self::Item>> {
        (**self).next()
    }
}

// Just a helper function to ensure the streams we're returning all have the
// right implementations.
pub(crate) fn assert_stream<T, S>(stream: S) -> S
where
    S: Stream<Item = T>,
{
    stream
}
