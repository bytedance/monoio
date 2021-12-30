// Forked from tokio.
use crate::net::unix::UnixStream;
use libc::{gid_t, pid_t, uid_t};
use std::{io, mem};

/// Credentials of a process
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct UCred {
    /// PID (process ID) of the process
    pid: Option<pid_t>,
    /// UID (user ID) of the process
    uid: uid_t,
    /// GID (group ID) of the process
    gid: gid_t,
}

impl UCred {
    /// Gets UID (user ID) of the process.
    pub fn uid(&self) -> uid_t {
        self.uid
    }

    /// Gets GID (group ID) of the process.
    pub fn gid(&self) -> gid_t {
        self.gid
    }

    /// Gets PID (process ID) of the process.
    pub fn pid(&self) -> Option<pid_t> {
        self.pid
    }
}

pub(crate) fn get_peer_cred(sock: &UnixStream) -> io::Result<UCred> {
    use std::os::unix::io::AsRawFd;

    unsafe {
        let raw_fd = sock.as_raw_fd();

        let mut ucred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };

        let ucred_size = mem::size_of::<libc::ucred>();

        // These paranoid checks should be optimized-out
        assert!(mem::size_of::<u32>() <= mem::size_of::<usize>());
        assert!(ucred_size <= u32::MAX as usize);

        let mut ucred_size = ucred_size as libc::socklen_t;

        let ret = libc::getsockopt(
            raw_fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut ucred as *mut libc::ucred as *mut libc::c_void,
            &mut ucred_size,
        );
        if ret == 0 && ucred_size as usize == mem::size_of::<libc::ucred>() {
            Ok(UCred {
                uid: ucred.uid,
                gid: ucred.gid,
                pid: Some(ucred.pid),
            })
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
