#[cfg(windows)]
use {
    crate::syscall,
    std::os::windows::prelude::AsRawSocket,
    windows_sys::Win32::Networking::WinSock::{
        accept, socklen_t, INVALID_SOCKET, SOCKADDR_STORAGE,
    },
};

use crate::driver::op::Accept;

/// Accept pool
pub(crate) struct AcceptPool<const N: usize> {
    #[cfg(unix)]
    free_sockaddrs: [Option<Box<(MaybeUninit<libc::sockaddr_storage>, libc::socklen_t)>>; N],
    // #[cfg(windows)]
    // addrs: [Box<(MaybeUninit<SOCKADDR_STORAGE>, socklen_t)>; N],
    // index of the last free element or usize::MAX when free list is empty
    // can't be more then isize::MAX due to std::alloc::Layout allocation limit
    last_free_index: Wrapping<usize>,
    // Requested Accepts - both submitted and not
    accepts: [Option<Accept>; N],
    last_unqueued_index: Wrapping<usize>,
    last_queued_index: Wrapping<usize>,
    completions: [CompletionMeta; N],
}

impl<const N: usize> AcceptPool {
    /// Accept a connection if free sockaddr storage is available
    ///
    /// User can provide
    pub fn maybe_accept(&mut self, fd: &SharedFd, user_data: usize) -> Option<io::Result<Self>> {
        #[cfg(unix)]
        let addr = self.acquire_sockaddr();

        // #[cfg(windows)]
        // let addr = Box::new((
        //     MaybeUninit::uninit(),
        //     size_of::<SOCKADDR_STORAGE>() as socklen_t,
        // ));

        Op::maybe_submit_with(Accept {
            fd: fd.clone(),
            addr,
        })
    }

    pub(crate) fn new() -> Self {
        let free_sockaddrs = {
            let mut list: [MaybeUninit<
                Option<Box<(MaybeUninit<libc::sockaddr_storage>, libc::socklen_t)>>,
            >; N] = MaybeUninit::uninit_array();
            for elem in &mut list {
                let maybe_uninit_buf =
                    Box::<(MaybeUninit<libc::sockaddr_storage>, libc::socklen_t)>::new_zeroed()?;
                // SAFETY:
                // just allocated
                let buf = unsafe { maybe_uninit_buf.assume_init() };
                _ = elem.write(Some(buf));
            }
            // SAFETY:
            // array was previously initialized
            unsafe { MaybeUninit::array_assume_init(list) }
        };
        Ok(Self {
            free_sockaddrs,
            last_free_index: Wrapping(N) - Wrapping(1),
        })
    }

    /// Acquire sockaddr_storage from the pool
    fn acquire_sockaddr(
        &mut self,
    ) -> Option<Box<(MaybeUninit<libc::sockaddr_storage>, libc::socklen_t)>> {
        if self.last_free_index.0 < self.free_sockaddrs.len() {
            #[expect(clippy::indexing_slicing, reason = "safe indexing due to len check")]
            let maybe_sockaddr = self.free_sockaddrs[self.last_free_index.0].take();
            self.last_free_index -= Wrapping(1);
            maybe_sockaddr
        } else {
            None
        }
    }

    /// Release sockaddr_storage to the pool
    fn release_sockaddr(&mut self, sockaddr_storage: Box<(MaybeUninit<libc::sockaddr_storage>)>) {
        let new_free_index = self.last_free_index + Wrapping(1);
        *self
            .free_list
            .get_mut(new_free_index.0)
            .expect("pool ops fit the backing array") = Some(sockaddr_storage);
    }
}
