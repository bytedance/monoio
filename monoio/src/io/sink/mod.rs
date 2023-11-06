//! Sink trait in GAT style.
mod sink_ext;

use std::future::Future;

pub use sink_ext::SinkExt;

/// A `Sink` is a value into which other values can be sent in pure async/await.
#[must_use = "sinks do nothing unless polled"]
pub trait Sink<Item> {
    /// The type of value produced by the sink when an error occurs.
    type Error;

    /// Send item.
    fn send(&mut self, item: Item) -> impl Future<Output = Result<(), Self::Error>>;

    /// Flush any remaining output from this sink.
    fn flush(&mut self) -> impl Future<Output = Result<(), Self::Error>>;

    /// Flush any remaining output and close this sink, if necessary.
    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>>;
}

impl<T, S: ?Sized + Sink<T>> Sink<T> for &mut S {
    type Error = S::Error;

    fn send(&mut self, item: T) -> impl Future<Output = Result<(), Self::Error>> {
        (**self).send(item)
    }

    fn flush(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        (**self).flush()
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        (**self).close()
    }
}
