//! Sink trait in GAT style.
mod sink_ext;

pub use sink_ext::SinkExt;

/// A `Sink` is a value into which other values can be sent in pure async/await.
#[must_use = "sinks do nothing unless polled"]
pub trait Sink<Item> {
    /// The type of value produced by the sink when an error occurs.
    type Error;

    /// Future representing the send result.
    type SendFuture<'a>: std::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'a,
        Item: 'a;

    /// Future representing the flush result.
    type FlushFuture<'a>: std::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    /// Future representing the close result.
    type CloseFuture<'a>: std::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    /// Send item.
    fn send<'a>(&'a mut self, item: Item) -> Self::SendFuture<'a>
    where
        Item: 'a;

    /// Flush any remaining output from this sink.
    fn flush(&mut self) -> Self::FlushFuture<'_>;

    /// Flush any remaining output and close this sink, if necessary.
    fn close(&mut self) -> Self::CloseFuture<'_>;
}

impl<T, S: ?Sized + Sink<T>> Sink<T> for &mut S {
    type Error = S::Error;

    type SendFuture<'a> = S::SendFuture<'a>
    where
        Self: 'a, T: 'a;

    type FlushFuture<'a> = S::FlushFuture<'a>
    where
        Self: 'a;

    type CloseFuture<'a> = S::CloseFuture<'a>
    where
        Self: 'a;

    fn send<'a>(&'a mut self, item: T) -> Self::SendFuture<'a>
    where
        T: 'a,
    {
        (**self).send(item)
    }

    fn flush(&mut self) -> Self::FlushFuture<'_> {
        (**self).flush()
    }

    fn close(&mut self) -> Self::CloseFuture<'_> {
        (**self).close()
    }
}
