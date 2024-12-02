use std::task::{Context, Poll, Waker};

use super::ready::{Direction, Ready};

pub(crate) struct ScheduledIo {
    readiness: Ready,

    /// Waker used for AsyncRead.
    reader: Option<Waker>,
    /// Waker used for AsyncWrite.
    writer: Option<Waker>,

    #[cfg(windows)]
    pub state: std::sync::Arc<std::sync::Mutex<super::legacy::iocp::SocketStateInner>>,
}

#[cfg(not(windows))]
impl Default for ScheduledIo {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduledIo {
    pub(crate) const fn new(
        #[cfg(windows)] state: std::sync::Arc<
            std::sync::Mutex<super::legacy::iocp::SocketStateInner>,
        >,
    ) -> Self {
        Self {
            readiness: Ready::EMPTY,
            reader: None,
            writer: None,
            #[cfg(windows)]
            state,
        }
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn set_writable(&mut self) {
        self.readiness |= Ready::WRITABLE;
    }

    #[inline]
    pub(crate) fn set_readiness(&mut self, f: impl Fn(Ready) -> Ready) {
        self.readiness = f(self.readiness);
    }

    #[inline]
    pub(crate) fn wake(&mut self, ready: Ready) {
        if ready.is_readable() {
            if let Some(waker) = self.reader.take() {
                waker.wake();
            }
        }
        if ready.is_writable() {
            if let Some(waker) = self.writer.take() {
                waker.wake();
            }
        }
    }

    #[inline]
    pub(crate) fn clear_readiness(&mut self, ready: Ready) {
        self.readiness = self.readiness - ready;
    }

    #[allow(clippy::needless_pass_by_ref_mut)]
    #[inline]
    pub(crate) fn poll_readiness(
        &mut self,
        cx: &mut Context<'_>,
        direction: Direction,
    ) -> Poll<Ready> {
        let ready = direction.mask() & self.readiness;
        if !ready.is_empty() {
            return Poll::Ready(ready);
        }
        self.set_waker(cx, direction);
        Poll::Pending
    }

    #[inline]
    pub(crate) fn set_waker(&mut self, cx: &mut Context<'_>, direction: Direction) {
        let slot = match direction {
            Direction::Read => &mut self.reader,
            Direction::Write => &mut self.writer,
        };
        match slot {
            Some(existing) => {
                if !existing.will_wake(cx.waker()) {
                    existing.clone_from(cx.waker());
                }
            }
            None => {
                *slot = Some(cx.waker().clone());
            }
        }
    }
}
