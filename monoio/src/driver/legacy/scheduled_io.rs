use std::task::{Context, Poll, Waker};

use super::ready::{Direction, Ready};

pub(crate) struct ScheduledIo {
    readiness: Ready,

    /// Waker used for AsyncRead.
    reader: Option<Waker>,
    /// Waker used for AsyncWrite.
    writer: Option<Waker>,
}

impl Default for ScheduledIo {
    fn default() -> Self {
        Self {
            readiness: Ready::EMPTY,
            reader: None,
            writer: None,
        }
    }
}

impl ScheduledIo {
    #[allow(unused)]
    pub(crate) fn set_writable(&mut self) {
        self.readiness |= Ready::WRITABLE;
    }

    pub(crate) fn set_readiness(&mut self, f: impl Fn(Ready) -> Ready) {
        self.readiness = f(self.readiness);
    }

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

    pub(crate) fn clear_readiness(&mut self, ready: Ready) {
        self.readiness = self.readiness - ready;
    }

    #[allow(clippy::needless_pass_by_ref_mut)]
    pub(crate) fn poll_readiness(
        &mut self,
        cx: &mut Context<'_>,
        direction: Direction,
    ) -> Poll<Ready> {
        let ready = direction.mask() & self.readiness;
        if !ready.is_empty() {
            return Poll::Ready(ready);
        }
        let slot = match direction {
            Direction::Read => &mut self.reader,
            Direction::Write => &mut self.writer,
        };
        match slot {
            Some(existing) => {
                if !existing.will_wake(cx.waker()) {
                    *existing = cx.waker().clone();
                }
            }
            None => {
                *slot = Some(cx.waker().clone());
            }
        }
        Poll::Pending
    }
}
