use std::future::Future;

use super::Sink;

/// Sink extensions.
pub trait SinkExt<T>: Sink<T> {
    /// Send and flush.
    fn send_and_flush(&mut self, item: T) -> impl Future<Output = Result<(), Self::Error>>;
}

impl<T, A> SinkExt<T> for A
where
    A: Sink<T>,
{
    async fn send_and_flush(&mut self, item: T) -> Result<(), Self::Error> {
        Sink::<T>::send(self, item).await?;
        Sink::<T>::flush(self).await
    }
}
