use std::{future::Future, io};

use monoio::BufResult;
use reusable_box_future::{ReusableBoxFuture, ReusableLocalBoxFuture};

use crate::buf::{Buf, RawBuf};

#[derive(Debug)]
pub struct MaybeArmedBoxFuture<T> {
    slot: ReusableLocalBoxFuture<T>,
    armed: bool,
}

impl<T> MaybeArmedBoxFuture<T> {
    pub fn armed(&self) -> bool {
        self.armed
    }

    pub fn arm_future<F>(&mut self, f: F)
    where
        F: Future<Output = T> + 'static,
    {
        self.armed = true;
        self.slot.set(f);
    }

    pub fn poll(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<T> {
        match self.slot.poll(cx) {
            r @ std::task::Poll::Ready(_) => {
                self.armed = false;
                r
            }
            p => p,
        }
    }

    pub fn new<F>(f: F) -> Self
    where
        F: Future<Output = T> + 'static,
    {
        Self {
            slot: ReusableLocalBoxFuture::new(f),
            armed: false,
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

#[derive(Debug)]
pub struct SendableMaybeArmedBoxFuture<T> {
    slot: ReusableBoxFuture<T>,
    armed: bool,
}

impl<T> SendableMaybeArmedBoxFuture<T> {
    pub fn armed(&self) -> bool {
        self.armed
    }
    pub fn arm_future<F>(&mut self, f: F)
    where
        F: Future<Output = T> + 'static + Send,
    {
        self.armed = true;
        self.slot.set(f);
    }
    pub fn poll(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<T> {
        match self.slot.poll(cx) {
            r @ std::task::Poll::Ready(_) => {
                self.armed = false;
                r
            }
            p => p,
        }
    }
    pub fn new<F>(f: F) -> Self
    where
        F: Future<Output = T> + 'static + Send,
    {
        Self {
            slot: ReusableBoxFuture::new(f),
            armed: false,
        }
    }
}
