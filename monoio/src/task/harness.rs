use std::{
    future::Future,
    panic,
    ptr::NonNull,
    task::{Context, Poll, Waker},
};

use super::utils::UnsafeCellExt;
use crate::{
    task::{
        core::{Cell, Core, CoreStage, Header, Trailer},
        state::Snapshot,
        waker::waker_ref,
        Schedule, Task,
    },
    utils::thread_id::{try_get_current_thread_id, DEFAULT_THREAD_ID},
};

pub(crate) struct Harness<T: Future, S: 'static> {
    cell: NonNull<Cell<T, S>>,
}

impl<T, S> Harness<T, S>
where
    T: Future,
    S: 'static,
{
    pub(crate) unsafe fn from_raw(ptr: NonNull<Header>) -> Harness<T, S> {
        Harness {
            cell: ptr.cast::<Cell<T, S>>(),
        }
    }

    fn header(&self) -> &Header {
        unsafe { &self.cell.as_ref().header }
    }

    fn trailer(&self) -> &Trailer {
        unsafe { &self.cell.as_ref().trailer }
    }

    fn core(&self) -> &Core<T, S> {
        unsafe { &self.cell.as_ref().core }
    }
}

impl<T, S> Harness<T, S>
where
    T: Future,
    S: Schedule,
{
    /// Polls the inner future.
    pub(super) fn poll(self) {
        trace!("MONOIO DEBUG[Harness]:: poll");
        match self.poll_inner() {
            PollFuture::Notified => {
                // We should re-schedule the task.
                self.header().state.ref_inc();
                self.core().scheduler.yield_now(self.get_new_task());
            }
            PollFuture::Complete => {
                self.complete();
            }
            PollFuture::Done => (),
        }
    }

    /// Do polland return the status.
    ///
    /// poll_inner does not take a ref-count. We must make sure the task is
    /// alive when call this method
    fn poll_inner(&self) -> PollFuture {
        // notified -> running
        self.header().state.transition_to_running();

        // poll the future
        let waker_ref = waker_ref::<T, S>(self.header());
        let cx = Context::from_waker(&waker_ref);
        let res = poll_future(&self.core().stage, cx);

        if res == Poll::Ready(()) {
            return PollFuture::Complete;
        }

        use super::state::TransitionToIdle;
        match self.header().state.transition_to_idle() {
            TransitionToIdle::Ok => PollFuture::Done,
            TransitionToIdle::OkNotified => PollFuture::Notified,
        }
    }

    pub(super) fn dealloc(self) {
        trace!("MONOIO DEBUG[Harness]:: dealloc");

        // Release the join waker, if there is one.
        self.trailer().waker.with_mut(drop);

        // Check causality
        self.core().stage.with_mut(drop);

        unsafe {
            drop(Box::from_raw(self.cell.as_ptr()));
        }
    }

    #[cfg(feature = "sync")]
    pub(super) fn finish(self, val: <T as Future>::Output) {
        trace!("MONOIO DEBUG[Harness]:: finish");
        self.header().state.transition_to_running();
        self.core().stage.store_output(val);
        self.complete();
    }

    // ===== join handle =====

    /// Read the task output into `dst`.
    pub(super) fn try_read_output(self, dst: &mut Poll<T::Output>, waker: &Waker) {
        trace!("MONOIO DEBUG[Harness]:: try_read_output");
        if can_read_output(self.header(), self.trailer(), waker) {
            *dst = Poll::Ready(self.core().stage.take_output());
        }
    }

    pub(super) fn drop_join_handle_slow(self) {
        trace!("MONOIO DEBUG[Harness]:: drop_join_handle_slow");

        let mut maybe_panic = None;

        // Try to unset `JOIN_INTEREST`. This must be done as a first step in
        // case the task concurrently completed.
        if self.header().state.unset_join_interested().is_err() {
            // It is our responsibility to drop the output. This is critical as
            // the task output may not be `Send` and as such must remain with
            // the scheduler or `JoinHandle`. i.e. if the output remains in the
            // task structure until the task is deallocated, it may be dropped
            // by a Waker on any arbitrary thread.
            let panic = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                self.core().stage.drop_future_or_output();
            }));

            if let Err(panic) = panic {
                maybe_panic = Some(panic);
            }
        }

        // Drop the `JoinHandle` reference, possibly deallocating the task
        self.drop_reference();

        if let Some(panic) = maybe_panic {
            panic::resume_unwind(panic);
        }
    }

    // ===== waker behavior =====

    /// This call consumes a ref-count and notifies the task. This will create a
    /// new Notified and submit it if necessary.
    ///
    /// The caller does not need to hold a ref-count besides the one that was
    /// passed to this call.
    pub(super) fn wake_by_val(self) {
        trace!("MONOIO DEBUG[Harness]:: wake_by_val");
        let owner_id = self.header().owner_id;
        if is_remote_task(owner_id) {
            // send to target thread
            trace!("MONOIO DEBUG[Harness]:: wake_by_val with another thread id");
            #[cfg(feature = "sync")]
            {
                use crate::task::waker::raw_waker;
                let waker = raw_waker::<T, S>(self.cell.cast::<Header>().as_ptr());
                // # Ref Count: self -> waker
                let waker = unsafe { Waker::from_raw(waker) };
                crate::runtime::CURRENT.try_with(|maybe_ctx| match maybe_ctx {
                    Some(ctx) => {
                        ctx.send_waker(owner_id, waker);
                        ctx.unpark_thread(owner_id);
                    }
                    None => {
                        crate::runtime::DEFAULT_CTX.with(|default_ctx| {
                            crate::runtime::CURRENT.set(default_ctx, || {
                                crate::runtime::CURRENT.with(|ctx| {
                                    ctx.send_waker(owner_id, waker);
                                    ctx.unpark_thread(owner_id);
                                });
                            });
                        });
                    }
                });
                return;
            }
            #[cfg(not(feature = "sync"))]
            {
                panic!("waker can only be sent across threads when `sync` feature enabled");
            }
        }

        use super::state::TransitionToNotified;

        match self.header().state.transition_to_notified() {
            TransitionToNotified::Submit => {
                // # Ref Count: self -> task
                self.core().scheduler.schedule(self.get_new_task());
            }
            TransitionToNotified::DoNothing => {
                // # Ref Count: self -> -1
                self.drop_reference();
            }
        }
    }

    /// This call notifies the task. It will not consume any ref-counts, but the
    /// caller should hold a ref-count.  This will create a new Notified and
    /// submit it if necessary.
    pub(super) fn wake_by_ref(&self) {
        trace!("MONOIO DEBUG[Harness]:: wake_by_ref");
        let owner_id = self.header().owner_id;
        if is_remote_task(owner_id) {
            // send to target thread
            trace!("MONOIO DEBUG[Harness]:: wake_by_ref with another thread id");
            #[cfg(feature = "sync")]
            {
                use crate::task::waker::raw_waker;
                let waker = raw_waker::<T, S>(self.cell.cast::<Header>().as_ptr());
                // We create a new waker so we need to inc ref count.
                let waker = unsafe { Waker::from_raw(waker) };
                self.header().state.ref_inc();
                crate::runtime::CURRENT.try_with(|maybe_ctx| match maybe_ctx {
                    Some(ctx) => {
                        ctx.send_waker(owner_id, waker);
                        ctx.unpark_thread(owner_id);
                    }
                    None => {
                        crate::runtime::DEFAULT_CTX.with(|default_ctx| {
                            crate::runtime::CURRENT.set(default_ctx, || {
                                crate::runtime::CURRENT.with(|ctx| {
                                    ctx.send_waker(owner_id, waker);
                                    ctx.unpark_thread(owner_id);
                                });
                            });
                        });
                    }
                });
                return;
            }
            #[cfg(not(feature = "sync"))]
            {
                panic!("waker can only be sent across threads when `sync` feature enabled");
            }
        }

        use super::state::TransitionToNotified;

        match self.header().state.transition_to_notified() {
            TransitionToNotified::Submit => {
                // # Ref Count: +1 -> task
                self.header().state.ref_inc();
                self.core().scheduler.schedule(self.get_new_task());
            }
            TransitionToNotified::DoNothing => (),
        }
    }

    pub(super) fn drop_reference(self) {
        trace!("MONOIO DEBUG[Harness]:: drop_reference");
        if self.header().state.ref_dec() {
            self.dealloc();
        }
    }

    // ====== internal ======

    /// Complete the task. This method assumes that the state is RUNNING.
    fn complete(self) {
        // The future has completed and its output has been written to the task
        // stage. We transition from running to complete.

        let snapshot = self.header().state.transition_to_complete();

        // We catch panics here in case dropping the future or waking the
        // JoinHandle panics.
        let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            if !snapshot.is_join_interested() {
                // The `JoinHandle` is not interested in the output of
                // this task. It is our responsibility to drop the
                // output.
                self.core().stage.drop_future_or_output();
            } else if snapshot.has_join_waker() {
                // Notify the join handle. The previous transition obtains the
                // lock on the waker cell.
                self.trailer().wake_join();
            }
        }));
    }

    /// Create a new task that holds its own ref-count.
    ///
    /// # Safety
    ///
    /// Any use of `self` after this call must ensure that a ref-count to the
    /// task holds the task alive until after the use of `self`. Passing the
    /// returned Task to any method on `self` is unsound if dropping the Task
    /// could drop `self` before the call on `self` returned.
    fn get_new_task(&self) -> Task<S> {
        // safety: The header is at the beginning of the cell, so this cast is
        // safe.
        unsafe { Task::from_raw(self.cell.cast()) }
    }
}

fn is_remote_task(owner_id: usize) -> bool {
    if owner_id == DEFAULT_THREAD_ID {
        return true;
    }
    match try_get_current_thread_id() {
        Some(tid) => owner_id != tid,
        None => true,
    }
}

fn can_read_output(header: &Header, trailer: &Trailer, waker: &Waker) -> bool {
    // Load a snapshot of the current task state
    let snapshot = header.state.load();

    debug_assert!(snapshot.is_join_interested());

    if !snapshot.is_complete() {
        // The waker must be stored in the task struct.
        let res = if snapshot.has_join_waker() {
            // There already is a waker stored in the struct. If it matches
            // the provided waker, then there is no further work to do.
            // Otherwise, the waker must be swapped.
            let will_wake = unsafe {
                // Safety: when `JOIN_INTEREST` is set, only `JOIN_HANDLE`
                // may mutate the `waker` field.
                trailer.will_wake(waker)
            };

            if will_wake {
                // The task is not complete **and** the waker is up to date,
                // there is nothing further that needs to be done.
                return false;
            }

            // Unset the `JOIN_WAKER` to gain mutable access to the `waker`
            // field then update the field with the new join worker.
            //
            // This requires two atomic operations, unsetting the bit and
            // then resetting it. If the task transitions to complete
            // concurrently to either one of those operations, then setting
            // the join waker fails and we proceed to reading the task
            // output.
            header
                .state
                .unset_waker()
                .and_then(|snapshot| set_join_waker(header, trailer, waker.clone(), snapshot))
        } else {
            set_join_waker(header, trailer, waker.clone(), snapshot)
        };

        match res {
            Ok(_) => return false,
            Err(snapshot) => {
                assert!(snapshot.is_complete());
            }
        }
    }
    true
}

fn set_join_waker(
    header: &Header,
    trailer: &Trailer,
    waker: Waker,
    snapshot: Snapshot,
) -> Result<Snapshot, Snapshot> {
    assert!(snapshot.is_join_interested());
    assert!(!snapshot.has_join_waker());

    // Safety: Only the `JoinHandle` may set the `waker` field. When
    // `JOIN_INTEREST` is **not** set, nothing else will touch the field.
    unsafe {
        trailer.set_waker(Some(waker));
    }

    // Update the `JoinWaker` state accordingly
    let res = header.state.set_join_waker();

    // If the state could not be updated, then clear the join waker
    if res.is_err() {
        unsafe {
            trailer.set_waker(None);
        }
    }

    res
}

enum PollFuture {
    Complete,
    Notified,
    Done,
}

/// Poll the future. If the future completes, the output is written to the
/// stage field.
fn poll_future<T: Future>(core: &CoreStage<T>, cx: Context<'_>) -> Poll<()> {
    // CHIHAI: For efficiency we do not catch.

    // Poll the future.
    // let output = panic::catch_unwind(panic::AssertUnwindSafe(|| {
    //     struct Guard<'a, T: Future> {
    //         core: &'a CoreStage<T>,
    //     }
    //     impl<'a, T: Future> Drop for Guard<'a, T> {
    //         fn drop(&mut self) {
    //             // If the future panics on poll, we drop it inside the panic
    //             // guard.
    //             self.core.drop_future_or_output();
    //         }
    //     }
    //     let guard = Guard { core };
    //     let res = guard.core.poll(cx);
    //     mem::forget(guard);
    //     res
    // }));
    let output = core.poll(cx);

    // Prepare output for being placed in the core stage.
    let output = match output {
        // Ok(Poll::Pending) => return Poll::Pending,
        // Ok(Poll::Ready(output)) => Ok(output),
        // Err(panic) => Err(JoinError::panic(panic)),
        Poll::Pending => return Poll::Pending,
        Poll::Ready(output) => output,
    };

    // Catch and ignore panics if the future panics on drop.
    // let _ = panic::catch_unwind(panic::AssertUnwindSafe(|| {
    //     core.store_output(output);
    // }));
    core.store_output(output);

    Poll::Ready(())
}
