#![allow(unreachable_pub)]
//! Unix related.

mod listener;
mod socket_addr;
mod stream;

pub use listener::UnixListener;
pub use socket_addr::SocketAddr;
pub use stream::UnixStream;

pub(crate) fn path_offset(sockaddr: &libc::sockaddr_un) -> usize {
    let base = sockaddr as *const _ as usize;
    let path = &sockaddr.sun_path as *const _ as usize;
    path - base
}
