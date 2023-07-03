use crate::driver::unpark::Unpark;

pub(crate) struct EventWaker {
    // raw waker
    #[cfg(windows)]
    waker: super::iocp::Waker,
    #[cfg(unix)]
    waker: mio::Waker,
    // Atomic awake status
    pub(crate) awake: std::sync::atomic::AtomicBool,
}

impl EventWaker {
    #[cfg(unix)]
    pub(crate) fn new(waker: mio::Waker) -> Self {
        Self {
            waker,
            awake: std::sync::atomic::AtomicBool::new(true),
        }
    }

    #[cfg(windows)]
    pub(crate) fn new(waker: super::iocp::Waker) -> Self {
        Self {
            waker,
            awake: std::sync::atomic::AtomicBool::new(true),
        }
    }

    pub(crate) fn wake(&self) -> std::io::Result<()> {
        // Skip wake if already awake
        if self.awake.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        self.waker.wake()
    }
}

#[derive(Clone)]
pub struct UnparkHandle(pub(crate) std::sync::Weak<EventWaker>);

impl Unpark for UnparkHandle {
    fn unpark(&self) -> std::io::Result<()> {
        if let Some(w) = self.0.upgrade() {
            w.wake()
        } else {
            Ok(())
        }
    }
}
