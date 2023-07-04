pub(crate) trait IntoInner<T> {
    /// Consumes the allocation, returning the value.
    fn consume(self) -> T;
}

impl<T> IntoInner<T> for Box<T> {
    #[inline]
    fn consume(self) -> T {
        *self
    }
}
