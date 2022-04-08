use std::{
    io,
    mem::{size_of, MaybeUninit},
};

use super::{Op, SharedFd};

/// Accept
pub(crate) struct Accept {
    #[allow(unused)]
    pub(crate) fd: SharedFd,
    pub(crate) addr: MaybeUninit<libc::sockaddr_storage>,
    pub(crate) addrlen: libc::socklen_t,
}

impl Op<Accept> {
    /// Accept a connection
    pub(crate) fn accept(fd: &SharedFd) -> io::Result<Self> {
        use io_uring::{opcode, types};
        Op::submit_with(
            Accept {
                fd: fd.clone(),
                addr: MaybeUninit::uninit(),
                addrlen: size_of::<libc::sockaddr_storage>() as libc::socklen_t,
            },
            |accept| {
                opcode::Accept::new(
                    types::Fd(fd.raw_fd()),
                    accept.addr.as_mut_ptr() as *mut _,
                    &mut accept.addrlen,
                )
                .build()
            },
        )
    }
}
