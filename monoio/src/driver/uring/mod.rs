//! Monoio Uring Driver.

use std::{
    cell::UnsafeCell,
    io,
    mem::ManuallyDrop,
    os::unix::prelude::{AsRawFd, RawFd},
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

use io_uring::{cqueue, opcode, types::Timespec, IoUring};
use lifecycle::Lifecycle;

use super::{
    op::{CompletionMeta, Op, OpAble},
    util::timespec,
    Driver, Inner, CURRENT,
};
use crate::utils::slab::Slab;

mod lifecycle;
#[cfg(feature = "sync")]
mod waker;
#[cfg(feature = "sync")]
pub(crate) use waker::UnparkHandle;

#[allow(unused)]
pub(crate) const CANCEL_USERDATA: u64 = u64::MAX;
pub(crate) const TIMEOUT_USERDATA: u64 = u64::MAX - 1;
#[allow(unused)]
pub(crate) const EVENTFD_USERDATA: u64 = u64::MAX - 2;

pub(crate) const MIN_REVERSED_USERDATA: u64 = u64::MAX - 2;

/// Driver with uring.
pub struct IoUringDriver {
    inner: Rc<UnsafeCell<UringInner>>,

    // Used as timeout buffer
    timespec: *mut Timespec,

    // Used as read eventfd buffer
    #[cfg(feature = "sync")]
    eventfd_read_dst: *mut u8,

    // Used for drop
    #[cfg(feature = "sync")]
    thread_id: usize,
}

pub(crate) struct UringInner {
    /// In-flight operations
    ops: Ops,

    /// IoUring bindings
    uring: ManuallyDrop<IoUring>,

    /// Shared waker
    #[cfg(feature = "sync")]
    shared_waker: std::sync::Arc<waker::EventWaker>,

    // Mark if eventfd is in the ring
    #[cfg(feature = "sync")]
    eventfd_installed: bool,

    // Waker receiver
    #[cfg(feature = "sync")]
    waker_receiver: flume::Receiver<std::task::Waker>,
}

// When dropping the driver, all in-flight operations must have completed. This
// type wraps the slab and ensures that, on drop, the slab is empty.
struct Ops {
    slab: Slab<Lifecycle>,
}

impl IoUringDriver {
    const DEFAULT_ENTRIES: u32 = 1024;

    pub(crate) fn new() -> io::Result<IoUringDriver> {
        Self::new_with_entries(Self::DEFAULT_ENTRIES)
    }

    #[cfg(not(feature = "sync"))]
    pub(crate) fn new_with_entries(entries: u32) -> io::Result<IoUringDriver> {
        let uring = ManuallyDrop::new(IoUring::new(entries)?);

        let inner = Rc::new(UnsafeCell::new(UringInner {
            ops: Ops::new(),
            uring,
        }));

        Ok(IoUringDriver {
            inner,
            timespec: Box::leak(Box::new(Timespec::new())) as *mut Timespec,
        })
    }

    #[cfg(feature = "sync")]
    pub(crate) fn new_with_entries(entries: u32) -> io::Result<IoUringDriver> {
        let uring = ManuallyDrop::new(IoUring::new(entries)?);

        // Create eventfd and register it to the ring.
        let waker = {
            let fd = crate::syscall!(eventfd(0, libc::EFD_CLOEXEC))?;
            unsafe {
                use std::os::unix::io::FromRawFd;
                std::fs::File::from_raw_fd(fd)
            }
        };

        let (waker_sender, waker_receiver) = flume::unbounded::<std::task::Waker>();

        let inner = Rc::new(UnsafeCell::new(UringInner {
            ops: Ops::new(),
            uring,
            shared_waker: std::sync::Arc::new(waker::EventWaker::new(waker)),
            eventfd_installed: false,
            waker_receiver,
        }));

        let thread_id = crate::builder::BUILD_THREAD_ID.with(|id| *id);
        let driver = IoUringDriver {
            inner,
            timespec: Box::leak(Box::new(Timespec::new())) as *mut Timespec,
            eventfd_read_dst: Box::leak(Box::new([0_u8; 8])) as *mut u8,
            thread_id,
        };

        // Register unpark handle
        super::thread::register_unpark_handle(thread_id, driver.unpark().into());
        super::thread::register_waker_sender(thread_id, waker_sender);
        Ok(driver)
    }

    #[allow(unused)]
    fn num_operations(&self) -> usize {
        let inner = self.inner.get();
        unsafe { (*inner).ops.slab.len() }
    }

    // Flush to make enough space
    fn flush_space(inner: &mut UringInner, need: usize) -> io::Result<()> {
        let sq = inner.uring.submission();
        debug_assert!(sq.capacity() >= need);
        if sq.len() + need > sq.capacity() {
            drop(sq);
            inner.submit()?;
        }
        Ok(())
    }

    #[cfg(feature = "sync")]
    fn install_eventfd(&self, inner: &mut UringInner, fd: RawFd) {
        let entry = opcode::Read::new(io_uring::types::Fd(fd), self.eventfd_read_dst, 8)
            .build()
            .user_data(EVENTFD_USERDATA);

        let mut sq = inner.uring.submission();
        let _ = unsafe { sq.push(&entry) };
        inner.eventfd_installed = true;
    }

    fn install_timeout(&self, inner: &mut UringInner, duration: Duration) {
        let timespec = timespec(duration);
        unsafe {
            std::ptr::replace(self.timespec, timespec);
        }
        let entry = opcode::Timeout::new(self.timespec as *const Timespec)
            .build()
            .user_data(TIMEOUT_USERDATA);

        let mut sq = inner.uring.submission();
        let _ = unsafe { sq.push(&entry) };
    }

    fn inner_park(&self, timeout: Option<Duration>) -> io::Result<()> {
        let inner = unsafe { &mut *self.inner.get() };

        #[allow(unused_mut)]
        let mut need_wait = true;

        #[cfg(feature = "sync")]
        {
            // Process foreign wakers
            while let Ok(w) = inner.waker_receiver.try_recv() {
                w.wake();
                need_wait = false;
            }

            // Set status as not awake if we are going to sleep
            if need_wait {
                inner
                    .shared_waker
                    .awake
                    .store(false, std::sync::atomic::Ordering::Release);
            }

            // Process foreign wakers left
            while let Ok(w) = inner.waker_receiver.try_recv() {
                w.wake();
                need_wait = false;
            }
        }

        if need_wait {
            // Install timeout and eventfd for unpark if sync is enabled

            // 1. alloc spaces
            let mut space = 0;
            #[cfg(feature = "sync")]
            if !inner.eventfd_installed {
                space += 1;
            }
            if timeout.is_some() {
                space += 1;
            }
            if space != 0 {
                Self::flush_space(inner, space)?;
            }

            // 2. install eventfd and timeout
            #[cfg(feature = "sync")]
            if !inner.eventfd_installed {
                self.install_eventfd(inner, inner.shared_waker.as_raw_fd());
            }
            if let Some(duration) = timeout {
                self.install_timeout(inner, duration);
            }

            // Submit and Wait
            inner.uring.submit_and_wait(1)?;
        } else {
            // Submit only
            inner.uring.submit()?;
        }

        // Set status as awake
        #[cfg(feature = "sync")]
        inner
            .shared_waker
            .awake
            .store(true, std::sync::atomic::Ordering::Release);

        // Process CQ
        inner.tick();

        Ok(())
    }
}

impl Driver for IoUringDriver {
    /// Enter the driver context. This enables using uring types.
    fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        // TODO(ihciah): remove clone
        let inner = Inner::Uring(self.inner.clone());
        CURRENT.set(&inner, f)
    }

    fn submit(&self) -> io::Result<()> {
        let inner = unsafe { &mut *self.inner.get() };
        inner.submit()?;
        inner.tick();
        Ok(())
    }

    fn park(&self) -> io::Result<()> {
        self.inner_park(None)
    }

    fn park_timeout(&self, duration: Duration) -> io::Result<()> {
        self.inner_park(Some(duration))
    }

    #[cfg(feature = "sync")]
    type Unpark = waker::UnparkHandle;

    #[cfg(feature = "sync")]
    fn unpark(&self) -> Self::Unpark {
        let weak = unsafe { std::sync::Arc::downgrade(&((*self.inner.get()).shared_waker)) };
        waker::UnparkHandle(weak)
    }
}

impl UringInner {
    fn tick(&mut self) {
        let mut cq = self.uring.completion();
        cq.sync();

        for cqe in cq {
            if cqe.user_data() >= MIN_REVERSED_USERDATA {
                #[cfg(feature = "sync")]
                if cqe.user_data() == EVENTFD_USERDATA {
                    self.eventfd_installed = false;
                }
                continue;
            }
            let index = cqe.user_data() as _;
            self.ops.complete(index, resultify(&cqe), cqe.flags());
        }
    }

    fn submit(&mut self) -> io::Result<()> {
        loop {
            match self.uring.submit() {
                Ok(_) => {
                    self.uring.submission().sync();
                    return Ok(());
                }
                Err(ref e)
                    if e.kind() == io::ErrorKind::Other
                        || e.kind() == io::ErrorKind::ResourceBusy =>
                {
                    self.tick();
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    fn new_op<T>(data: T, inner: &mut UringInner, driver: Inner) -> Op<T> {
        Op {
            driver,
            index: inner.ops.insert(),
            data: Some(data),
        }
    }

    pub(crate) fn submit_with_data<T>(
        this: &Rc<UnsafeCell<UringInner>>,
        data: T,
    ) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        let inner = unsafe { &mut *this.get() };
        // If the submission queue is full, flush it to the kernel
        if inner.uring.submission().is_full() {
            inner.submit()?;
        }

        // Create the operation
        let mut op = Self::new_op(data, inner, Inner::Uring(this.clone()));

        // Configure the SQE
        let data_mut = unsafe { op.data.as_mut().unwrap_unchecked() };
        let sqe = OpAble::uring_op(data_mut).user_data(op.index as _);

        {
            let mut sq = inner.uring.submission();

            // Push the new operation
            if unsafe { sq.push(&sqe).is_err() } {
                unimplemented!("when is this hit?");
            }
        }

        // Submit the new operation. At this point, the operation has been
        // pushed onto the queue and the tail pointer has been updated, so
        // the submission entry is visible to the kernel. If there is an
        // error here (probably EAGAIN), we still return the operation. A
        // future `io_uring_enter` will fully submit the event.

        // CHIHAI: We are not going to do syscall now. If we are waiting
        // for IO, we will submit on `park`.
        // let _ = inner.submit();
        Ok(op)
    }

    pub(crate) fn poll_op(
        this: &Rc<UnsafeCell<UringInner>>,
        index: usize,
        cx: &mut Context<'_>,
    ) -> Poll<CompletionMeta> {
        let inner = unsafe { &mut *this.get() };
        let lifecycle = unsafe { inner.ops.slab.get(index).unwrap_unchecked() };
        lifecycle.poll_op(cx)
    }

    pub(crate) fn drop_op<T: 'static>(
        this: &Rc<UnsafeCell<UringInner>>,
        index: usize,
        data: &mut Option<T>,
    ) {
        let inner = unsafe { &mut *this.get() };
        if index == usize::MAX {
            // already finished
            return;
        }
        if let Some(lifecycle) = inner.ops.slab.get(index) {
            let _must_finished = lifecycle.drop_op(data);
            #[cfg(features = "async-cancel")]
            if !_must_finished {
                unsafe {
                    let cancel = io_uring::opcode::AsyncCancel::new(index as u64).build();

                    // Try push cancel, if failed, will submit and re-push.
                    if inner.uring.submission().push(&cancel).is_err() {
                        let _ = inner.submit();
                        let _ = inner.uring.submission().push(&cancel);
                    }
                }
            }
        }
    }
}

impl AsRawFd for IoUringDriver {
    fn as_raw_fd(&self) -> RawFd {
        unsafe { (*self.inner.get()).uring.as_raw_fd() }
    }
}

impl Drop for IoUringDriver {
    fn drop(&mut self) {
        trace!("MONOIO DEBUG[IoUringDriver]: drop");

        // Dealloc leaked memory
        unsafe { std::ptr::drop_in_place(self.timespec) };

        #[cfg(feature = "sync")]
        unsafe {
            std::ptr::drop_in_place(self.eventfd_read_dst)
        };

        // Deregister thread id
        #[cfg(feature = "sync")]
        {
            use crate::driver::thread::{unregister_unpark_handle, unregister_waker_sender};
            unregister_unpark_handle(self.thread_id);
            unregister_waker_sender(self.thread_id);
        }
    }
}

impl Drop for UringInner {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.uring);
        }
    }
}

impl Ops {
    const fn new() -> Self {
        Ops { slab: Slab::new() }
    }

    // Insert a new operation
    pub(crate) fn insert(&mut self) -> usize {
        self.slab.insert(Lifecycle::Submitted)
    }

    fn complete(&mut self, index: usize, result: io::Result<u32>, flags: u32) {
        let lifecycle = unsafe { self.slab.get(index).unwrap_unchecked() };
        lifecycle.complete(result, flags);
    }
}

#[inline]
fn resultify(cqe: &cqueue::Entry) -> io::Result<u32> {
    let res = cqe.result();

    if res >= 0 {
        Ok(res as u32)
    } else {
        Err(io::Error::from_raw_os_error(-res))
    }
}
