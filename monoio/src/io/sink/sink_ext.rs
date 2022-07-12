use super::Sink;

/// Sink extensions.
pub trait SinkExt<T>: Sink<T> {
    /// SendFlushFuture.
    type SendFlushFuture<'a>: std::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    /// Send and flush.
    fn send_and_flush(&mut self, item: T) -> Self::SendFlushFuture<'_>;
}

impl<T, A> SinkExt<T> for A
where
    A: Sink<T>,
{
    type SendFlushFuture<'a> = impl std::future::Future<Output = Result<(), Self::Error>> where A: 'a;

    fn send_and_flush(&mut self, item: T) -> Self::SendFlushFuture<'_> {
        async move {
            Sink::<T>::send(self, item).await?;
            Sink::<T>::flush(self).await
        }
    }
}
