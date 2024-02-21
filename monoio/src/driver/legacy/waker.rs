use mio::Token;

use crate::driver::unpark::Unpark;

#[cfg(unix)]
pub(crate) struct EventWaker {
    // raw waker
    waker: mio::Waker,
    // Atomic awake status
    pub(crate) awake: std::sync::atomic::AtomicBool,
}

#[cfg(unix)]
impl EventWaker {
    pub(crate) fn new(registry: &mio::Registry, token: Token) -> std::io::Result<Self> {
        Ok(Self {
            waker: mio::Waker::new(registry, token)?,
            awake: std::sync::atomic::AtomicBool::new(true),
        })
    }

    pub(crate) fn wake(&self) -> std::io::Result<()> {
        // Skip wake if already awake
        if self.awake.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        self.waker.wake()
    }
}

#[cfg(windows)]
pub(crate) struct EventWaker {
    // raw waker
    poll: std::sync::Arc<polling::Poller>,
    token: Token,
    // Atomic awake status
    pub(crate) awake: std::sync::atomic::AtomicBool,
}

#[cfg(windows)]
impl EventWaker {
    pub(crate) fn new(
        poll: std::sync::Arc<polling::Poller>,
        token: Token,
    ) -> std::io::Result<Self> {
        Ok(Self {
            poll,
            token,
            awake: std::sync::atomic::AtomicBool::new(true),
        })
    }

    pub(crate) fn wake(&self) -> std::io::Result<()> {
        use polling::os::iocp::PollerIocpExt;
        // Skip wake if already awake
        if self.awake.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        self.poll.post(polling::os::iocp::CompletionPacket::new(
            polling::Event::readable(self.token.0),
        ))
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
