use std::task::{Context, Poll, Waker};

use super::ready::{Direction, Ready};

pub(crate) struct ScheduledIo {
    readiness: Ready,

    /// Waker used for AsyncRead.
    r_waiter: Option<Waker>,
    /// Waker used for AsyncWrite.
    w_waiter: Option<Waker>,
    /// Waker requires read or write.
    rw_waiter: Option<Waker>,
}

impl Default for ScheduledIo {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduledIo {
    pub(crate) const fn new() -> Self {
        Self {
            readiness: Ready::EMPTY,
            r_waiter: None,
            w_waiter: None,
            rw_waiter: None,
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
        macro_rules! try_wake {
            ($w: expr) => {
                if let Some(waker) = $w.take() {
                    waker.wake();
                }
            };
        }
        match (ready.is_readable(), ready.is_writable()) {
            (true, true) => {
                try_wake!(self.r_waiter);
                try_wake!(self.w_waiter);
                try_wake!(self.rw_waiter);
            }
            (true, false) => {
                try_wake!(self.r_waiter);
                try_wake!(self.rw_waiter);
            }
            (false, true) => {
                try_wake!(self.w_waiter);
                try_wake!(self.rw_waiter);
            }
            _ => (),
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
        macro_rules! set_waker_slot {
            ($slot: expr) => {
                if let Some(existing) = $slot {
                    existing.clone_from(cx.waker());
                } else {
                    *$slot = Some(cx.waker().clone());
                }
            };
        }
        match direction {
            Direction::Read => set_waker_slot!(&mut self.r_waiter),
            Direction::Write => set_waker_slot!(&mut self.w_waiter),
            Direction::ReadOrWrite => set_waker_slot!(&mut self.rw_waiter),
        };
    }
}
