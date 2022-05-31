use std::{future::Future, io};

use monoio::BufResult;
use reusable_box_future::ReusableLocalBoxFuture;

use crate::buf::{Buf, RawBuf};

pub(crate) struct MaybeArmedBoxFuture<T> {
    slot: ReusableLocalBoxFuture<T>,
    armed: bool,
}

impl<T> MaybeArmedBoxFuture<T> {
    pub(crate) fn armed(&self) -> bool {
        self.armed
    }

    pub(crate) fn arm_future<F>(&mut self, f: F)
    where
        F: Future<Output = T> + 'static,
    {
        self.armed = true;
        self.slot.set(f);
    }

    pub(crate) fn poll(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<T> {
        match self.slot.poll(cx) {
            r @ std::task::Poll::Ready(_) => {
                self.armed = false;
                r
            }
            p => p,
        }
    }
}

impl Default for MaybeArmedBoxFuture<BufResult<usize, Buf>> {
    fn default() -> Self {
        Self {
            slot: ReusableLocalBoxFuture::new(async { (Ok(0), Buf::uninit()) }),
            armed: false,
        }
    }
}

impl Default for MaybeArmedBoxFuture<BufResult<usize, RawBuf>> {
    fn default() -> Self {
        Self {
            slot: ReusableLocalBoxFuture::new(async { (Ok(0), RawBuf::uninit()) }),
            armed: false,
        }
    }
}

impl<T> Default for MaybeArmedBoxFuture<BufResult<usize, T>>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            slot: ReusableLocalBoxFuture::new(async { (Ok(0), T::default()) }),
            armed: false,
        }
    }
}

impl Default for MaybeArmedBoxFuture<io::Result<()>> {
    fn default() -> Self {
        Self {
            slot: ReusableLocalBoxFuture::new(async { Ok(()) }),
            armed: false,
        }
    }
}
