use std::{
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};

use hyper::rt::{Executor, Sleep, Timer};
use pin_project_lite::pin_project;

#[derive(Clone, Copy, Debug)]
pub struct MonoioExecutor;
impl<F> Executor<F> for MonoioExecutor
where
    F: std::future::Future + 'static,
    F::Output: 'static,
{
    #[inline]
    fn execute(&self, fut: F) {
        monoio::spawn(fut);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MonoioTimer;
impl Timer for MonoioTimer {
    #[inline]
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Sleep>> {
        Box::pin(MonoioSleep {
            inner: monoio::time::sleep(duration),
        })
    }

    #[inline]
    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn Sleep>> {
        Box::pin(MonoioSleep {
            inner: monoio::time::sleep_until(deadline.into()),
        })
    }

    #[inline]
    fn reset(&self, sleep: &mut Pin<Box<dyn Sleep>>, new_deadline: Instant) {
        if let Some(sleep) = sleep.as_mut().downcast_mut_pin::<MonoioSleep>() {
            sleep.reset(new_deadline)
        }
    }
}

pin_project! {
    #[derive(Debug)]
    struct MonoioSleep {
        #[pin]
        inner: monoio::time::Sleep,
    }
}

impl Future for MonoioSleep {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

impl Sleep for MonoioSleep {}
unsafe impl Send for MonoioSleep {}
unsafe impl Sync for MonoioSleep {}

impl MonoioSleep {
    #[inline]
    pub fn reset(self: Pin<&mut Self>, deadline: Instant) {
        self.project().inner.as_mut().reset(deadline.into());
    }
}

pin_project! {
    #[derive(Debug)]
    pub struct MonoioIo<T> {
        #[pin]
        inner: T,
    }
}

impl<T> MonoioIo<T> {
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }

    #[inline]
    pub fn inner(self) -> T {
        self.inner
    }
}
impl<T> Deref for MonoioIo<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> DerefMut for MonoioIo<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl<T> hyper::rt::Read for MonoioIo<T>
where
    T: monoio::io::poll_io::AsyncRead,
{
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let n = unsafe {
            let mut tbuf = monoio::io::poll_io::ReadBuf::uninit(buf.as_mut());
            match monoio::io::poll_io::AsyncRead::poll_read(self.project().inner, cx, &mut tbuf) {
                Poll::Ready(Ok(())) => tbuf.filled().len(),
                other => return other,
            }
        };

        unsafe {
            buf.advance(n);
        }
        Poll::Ready(Ok(()))
    }
}

impl<T> hyper::rt::Write for MonoioIo<T>
where
    T: monoio::io::poll_io::AsyncWrite,
{
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        monoio::io::poll_io::AsyncWrite::poll_write(self.project().inner, cx, buf)
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        monoio::io::poll_io::AsyncWrite::poll_flush(self.project().inner, cx)
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        monoio::io::poll_io::AsyncWrite::poll_shutdown(self.project().inner, cx)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        monoio::io::poll_io::AsyncWrite::is_write_vectored(&self.inner)
    }

    #[inline]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        monoio::io::poll_io::AsyncWrite::poll_write_vectored(self.project().inner, cx, bufs)
    }
}

impl<T> tokio::io::AsyncRead for MonoioIo<T>
where
    T: hyper::rt::Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        tbuf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        // let init = tbuf.initialized().len();
        let filled = tbuf.filled().len();
        let sub_filled = unsafe {
            let mut buf = hyper::rt::ReadBuf::uninit(tbuf.unfilled_mut());

            match hyper::rt::Read::poll_read(self.project().inner, cx, buf.unfilled()) {
                Poll::Ready(Ok(())) => buf.filled().len(),
                other => return other,
            }
        };

        let n_filled = filled + sub_filled;
        // At least sub_filled bytes had to have been initialized.
        let n_init = sub_filled;
        unsafe {
            tbuf.assume_init(n_init);
            tbuf.set_filled(n_filled);
        }

        Poll::Ready(Ok(()))
    }
}

impl<T> tokio::io::AsyncWrite for MonoioIo<T>
where
    T: hyper::rt::Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        hyper::rt::Write::poll_write(self.project().inner, cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        hyper::rt::Write::poll_flush(self.project().inner, cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        hyper::rt::Write::poll_shutdown(self.project().inner, cx)
    }

    fn is_write_vectored(&self) -> bool {
        hyper::rt::Write::is_write_vectored(&self.inner)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        hyper::rt::Write::poll_write_vectored(self.project().inner, cx, bufs)
    }
}
