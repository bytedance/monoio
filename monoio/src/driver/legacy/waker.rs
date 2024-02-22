use mio::Token;

use crate::driver::unpark::Unpark;

pub(crate) struct EventWaker {
    // raw waker
    #[cfg(unix)]
    waker: mio::Waker,
    #[cfg(windows)]
    poll: std::sync::Arc<polling::Poller>,
    #[cfg(windows)]
    token: Token,
    // Atomic awake status
    pub(crate) awake: std::sync::atomic::AtomicBool,
}

impl EventWaker {
    #[cfg(unix)]
    pub(crate) fn new(registry: &mio::Registry, token: Token) -> std::io::Result<Self> {
        Ok(Self {
            waker: mio::Waker::new(registry, token)?,
            awake: std::sync::atomic::AtomicBool::new(true),
        })
    }

    #[cfg(windows)]
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
        // Skip wake if already awake
        if self.awake.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(());
        }
        #[cfg(unix)]
        let r = self.waker.wake();
        #[cfg(windows)]
        use polling::os::iocp::PollerIocpExt;
        #[cfg(windows)]
        let r = self.poll.post(polling::os::iocp::CompletionPacket::new(
            polling::Event::readable(self.token.0),
        ));
        r
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
