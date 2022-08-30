use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use crate::driver;

pub(crate) mod close;

mod accept;
mod connect;
mod fsync;
mod open;
mod read;
mod recv;
mod send;
mod write;

#[cfg(all(target_os = "linux", feature = "splice"))]
mod splice;

/// In-flight operation
pub(crate) struct Op<T: 'static> {
    // Driver running the operation
    pub(super) driver: driver::Inner,

    // Operation index in the slab(useless for legacy)
    pub(super) index: usize,

    // Per-operation data
    pub(super) data: Option<T>,
}

/// Operation completion. Returns stored state with the result of the operation.
#[derive(Debug)]
pub(crate) struct Completion<T> {
    pub(crate) data: T,
    pub(crate) meta: CompletionMeta,
}

/// Operation completion meta info.
#[derive(Debug)]
pub(crate) struct CompletionMeta {
    pub(crate) result: io::Result<u32>,
    #[allow(unused)]
    pub(crate) flags: u32,
}

pub(crate) trait OpAble {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry;

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(super::legacy::ready::Direction, usize)>;
    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32>;
}

impl<T> Op<T> {
    /// Submit an operation to uring.
    ///
    /// `state` is stored during the operation tracking any state submitted to
    /// the kernel.
    pub(super) fn submit_with(data: T) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        driver::CURRENT.with(|this| this.submit_with(data))
    }

    /// Try submitting an operation to uring
    #[allow(unused)]
    pub(super) fn try_submit_with(data: T) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        if driver::CURRENT.is_set() {
            Op::submit_with(data)
        } else {
            Err(io::ErrorKind::Other.into())
        }
    }
}

impl<T> Future for Op<T>
where
    T: Unpin + OpAble + 'static,
{
    type Output = Completion<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        let data_mut = me.data.as_mut().expect("unexpected operation state");
        let meta = ready!(me.driver.poll_op::<T>(data_mut, me.index, cx));

        me.index = usize::MAX;
        let data = me.data.take().expect("unexpected operation state");
        Poll::Ready(Completion { data, meta })
    }
}

impl<T> Drop for Op<T> {
    fn drop(&mut self) {
        self.driver.drop_op(self.index, &mut self.data);
    }
}

#[allow(unused)]
#[cfg(not(target_os = "linux"))]
pub(crate) fn non_blocking() -> bool {
    true
}

#[cfg(target_os = "linux")]
pub(crate) fn non_blocking() -> bool {
    super::CURRENT.with(|inner| inner.is_legacy())
}

// Copied from mio.
fn new_socket(domain: libc::c_int, socket_type: libc::c_int) -> io::Result<libc::c_int> {
    #[cfg(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    let socket_type = socket_type | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;

    #[cfg(target_os = "linux")]
    let socket_type = {
        if non_blocking() {
            socket_type | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK
        } else {
            socket_type | libc::SOCK_CLOEXEC
        }
    };

    // Gives a warning for platforms without SOCK_NONBLOCK.
    #[allow(clippy::let_and_return)]
    #[cfg(unix)]
    let socket = crate::syscall!(socket(domain, socket_type, 0));

    // Mimick `libstd` and set `SO_NOSIGPIPE` on apple systems.
    #[cfg(target_vendor = "apple")]
    let socket = socket.and_then(|socket| {
        crate::syscall!(setsockopt(
            socket,
            libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE,
            &1 as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t
        ))
        .map(|_| socket)
    });

    // Darwin doesn't have SOCK_NONBLOCK or SOCK_CLOEXEC.
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    let socket = socket.and_then(|socket| {
        // For platforms that don't support flags in socket, we need to
        // set the flags ourselves.
        crate::syscall!(fcntl(socket, libc::F_SETFL, libc::O_NONBLOCK))
            .and_then(|_| {
                crate::syscall!(fcntl(socket, libc::F_SETFD, libc::FD_CLOEXEC)).map(|_| socket)
            })
            .map_err(|e| {
                // If either of the `fcntl` calls failed, ensure the socket is
                // closed and return the error.
                let _ = crate::syscall!(close(socket));
                e
            })
    });

    #[cfg(windows)]
    let socket: std::io::Result<_> = unimplemented!();

    socket
}
