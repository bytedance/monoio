macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

#[allow(unused)]
pub(crate) const CANCEL_USERDATA: u64 = u64::MAX;
pub(crate) const TIMEOUT_USERDATA: u64 = u64::MAX - 1;
#[allow(unused)]
pub(crate) const EVENTFD_USERDATA: u64 = u64::MAX - 2;

pub(crate) const MIN_REVERSED_USERDATA: u64 = u64::MAX - 2;

pub(crate) mod op;
pub(crate) mod shared_fd;
mod util;

use crate::utils::slab::Slab;

use io_uring::{cqueue, IoUring};
use io_uring::{opcode, types::Timespec};
use scoped_tls::scoped_thread_local;
use std::cell::UnsafeCell;
use std::io;
use std::mem::ManuallyDrop;
use std::os::unix::io::{AsRawFd, RawFd};
use std::rc::Rc;
use std::time::Duration;

#[cfg(feature = "sync")]
pub(crate) mod thread;

use self::util::timespec;

#[allow(unreachable_pub)]
pub trait Unpark: Sync + Send + 'static {
    /// Unblocks a thread that is blocked by the associated `Park` handle.
    ///
    /// Calling `unpark` atomically makes available the unpark token, if it is
    /// not already available.
    ///
    /// # Panics
    ///
    /// This function **should** not panic, but ultimately, panics are left as
    /// an implementation detail. Refer to the documentation for the specific
    /// `Unpark` implementation
    fn unpark(&self);
}

impl Unpark for Box<dyn Unpark> {
    fn unpark(&self) {
        (**self).unpark()
    }
}

impl Unpark for std::sync::Arc<dyn Unpark> {
    fn unpark(&self) {
        (**self).unpark()
    }
}

pub trait Driver {
    fn with<R>(&self, f: impl FnOnce() -> R) -> R;
    fn submit(&self) -> io::Result<()>;
    fn park(&self) -> io::Result<()>;
    fn park_timeout(&self, duration: Duration) -> io::Result<()>;

    #[cfg(feature = "sync")]
    type Unpark: Unpark;

    #[cfg(feature = "sync")]
    fn unpark(&self) -> Self::Unpark;
}

pub struct IoUringDriver {
    inner: Handle,

    // Used as timeout buffer
    timespec: *mut Timespec,

    // Used as read eventfd buffer
    #[cfg(feature = "sync")]
    eventfd_read_dst: *mut u8,

    // Used for drop
    #[cfg(feature = "sync")]
    thread_id: usize,
}

type Handle = Rc<UnsafeCell<Inner>>;

struct Inner {
    /// In-flight operations
    ops: Ops,

    /// IoUring bindings
    uring: ManuallyDrop<IoUring>,

    /// Shared waker
    #[cfg(feature = "sync")]
    shared_waker: std::sync::Arc<EventWaker>,

    // Mark if eventfd is in the ring
    #[cfg(feature = "sync")]
    eventfd_installed: bool,

    // Waker receiver
    #[cfg(feature = "sync")]
    waker_receiver: flume::Receiver<std::task::Waker>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.uring);
        }
    }
}

// When dropping the driver, all in-flight operations must have completed. This
// type wraps the slab and ensures that, on drop, the slab is empty.
struct Ops {
    slab: Slab<op::Lifecycle>,
}

scoped_thread_local!(static CURRENT: Rc<UnsafeCell<Inner>>);

impl IoUringDriver {
    const DEFAULT_ENTRIES: u32 = 1024;

    pub(crate) fn new() -> io::Result<IoUringDriver> {
        Self::new_with_entries(Self::DEFAULT_ENTRIES)
    }

    #[cfg(not(feature = "sync"))]
    pub(crate) fn new_with_entries(entries: u32) -> io::Result<IoUringDriver> {
        let uring = IoUring::new(entries)?;

        let inner = Rc::new(UnsafeCell::new(Inner {
            ops: Ops::new(),
            uring: ManuallyDrop::new(uring),
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
            let fd = syscall!(eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK))?;
            unsafe {
                use std::os::unix::io::FromRawFd;
                std::fs::File::from_raw_fd(fd)
            }
        };

        let (waker_sender, waker_receiver) = flume::unbounded::<std::task::Waker>();

        let inner = Rc::new(UnsafeCell::new(Inner {
            ops: Ops::new(),
            uring,
            shared_waker: std::sync::Arc::new(EventWaker::new(waker)),
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
        self::thread::register_unpark_handle(thread_id, driver.unpark());
        self::thread::register_waker_sender(thread_id, waker_sender);
        Ok(driver)
    }

    #[allow(unused)]
    fn num_operations(&self) -> usize {
        let inner = self.inner.get();
        unsafe { (*inner).ops.slab.len() }
    }

    // Flush to make enough space
    fn flush_space(inner: &mut Inner, need: usize) -> io::Result<()> {
        let sq = inner.uring.submission();
        debug_assert!(sq.capacity() >= need);
        if sq.len() + need > sq.capacity() {
            drop(sq);
            inner.submit()?;
        }
        Ok(())
    }

    #[cfg(feature = "sync")]
    fn install_eventfd(&self, inner: &mut Inner, fd: RawFd) {
        let entry = opcode::Read::new(io_uring::types::Fd(fd), self.eventfd_read_dst, 8)
            .build()
            .user_data(EVENTFD_USERDATA);

        let mut sq = inner.uring.submission();
        let _ = unsafe { sq.push(&entry) };
        inner.eventfd_installed = true;
    }

    fn install_timeout(&self, inner: &mut Inner, duration: Duration) {
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
                self.install_eventfd(inner, inner.shared_waker.raw);
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
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Process CQ
        inner.tick();

        Ok(())
    }
}

impl Driver for IoUringDriver {
    /// Enter the driver context. This enables using uring types.
    fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        CURRENT.set(&self.inner, f)
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
    type Unpark = UnparkHandle;

    #[cfg(feature = "sync")]
    fn unpark(&self) -> Self::Unpark {
        let weak = unsafe { std::sync::Arc::downgrade(&((*self.inner.get()).shared_waker)) };
        UnparkHandle(weak)
    }
}

#[cfg(feature = "sync")]
pub(crate) struct EventWaker {
    // RawFd
    raw: RawFd,
    // File hold the ownership of fd, only useful when drop
    _file: std::fs::File,
    // Atomic awake status
    awake: std::sync::atomic::AtomicBool,
}

#[cfg(feature = "sync")]
impl EventWaker {
    fn new(file: std::fs::File) -> Self {
        Self {
            raw: file.as_raw_fd(),
            _file: file,
            awake: std::sync::atomic::AtomicBool::new(true),
        }
    }

    fn wake(&self) {
        // Skip wake if already awake
        if self.awake.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }
        // Write data into EventFd to wake the executor.
        let buf = 0x1u64.to_ne_bytes();
        unsafe {
            // SAFETY: Writing number to eventfd is thread safe.
            libc::write(self.raw, buf.as_ptr().cast(), buf.len());
        }
    }
}

#[cfg(feature = "sync")]
#[derive(Clone)]
pub struct UnparkHandle(std::sync::Weak<EventWaker>);

#[cfg(feature = "sync")]
impl Unpark for UnparkHandle {
    fn unpark(&self) {
        if let Some(w) = self.0.upgrade() {
            w.wake();
        }
    }
}

impl Inner {
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
}

impl AsRawFd for IoUringDriver {
    fn as_raw_fd(&self) -> RawFd {
        unsafe { (*self.inner.get()).uring.as_raw_fd() }
    }
}

impl Drop for IoUringDriver {
    fn drop(&mut self) {
        tracing!("MONOIO DEBUG[IoUringDriver]: drop");

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

impl Ops {
    const fn new() -> Self {
        Ops { slab: Slab::new() }
    }

    // Insert a new operation
    fn insert(&mut self) -> usize {
        self.slab.insert(op::Lifecycle::Submitted)
    }

    fn complete(&mut self, index: usize, result: io::Result<u32>, flags: u32) {
        let item = unsafe { self.slab.get_mut(index).unwrap_unchecked() };
        match item {
            op::Lifecycle::Submitted => {
                *item = op::Lifecycle::Completed(result, flags);
            }
            op::Lifecycle::Waiting(_) => {
                let old = std::mem::replace(item, op::Lifecycle::Completed(result, flags));
                match old {
                    op::Lifecycle::Waiting(waker) => {
                        waker.wake();
                    }
                    _ => unsafe { std::hint::unreachable_unchecked() },
                }
            }
            op::Lifecycle::Ignored(..) => {
                self.slab.remove(index);
            }
            op::Lifecycle::Completed(..) => unsafe { std::hint::unreachable_unchecked() },
        }
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
