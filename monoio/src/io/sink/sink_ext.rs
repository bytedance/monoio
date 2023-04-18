use super::Sink;

/// Sink extensions.
pub trait SinkExt<T>: Sink<T> {
    /// SendFlushFuture.
    type SendFlushFuture<'a>: std::future::Future<Output = Result<(), Self::Error>> + 'a
    where
        Self: 'a,
        T: 'a;

    /// Send and flush.
    fn send_and_flush<'a>(&'a mut self, item: T) -> Self::SendFlushFuture<'a>
    where
        T: 'a;
}

impl<T, A> SinkExt<T> for A
where
    A: Sink<T>,
{
    type SendFlushFuture<'a> = impl std::future::Future<Output = Result<(), Self::Error>> + 'a where
        A: 'a, T: 'a;

    fn send_and_flush<'a>(&'a mut self, item: T) -> Self::SendFlushFuture<'a>
    where
        T: 'a,
    {
        async move {
            Sink::<T>::send(self, item).await?;
            Sink::<T>::flush(self).await
        }
    }
}
