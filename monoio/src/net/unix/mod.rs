#![allow(unreachable_pub)]
//! Unix related.

mod datagram;
mod listener;
mod pipe;
mod socket_addr;
mod split;
mod stream;
mod ucred;

pub use datagram::UnixDatagram;
pub use listener::UnixListener;
pub use pipe::{new_pipe, Pipe};
pub use socket_addr::SocketAddr;
pub use split::{
    OwnedReadHalf as UnixOwnedReadHalf, OwnedWriteHalf as UnixOwnedWriteHalf,
    ReadHalf as UnixReadHalf, ReuniteError as UnixReuniteError, WriteHalf as UnixWriteHalf,
};
pub use stream::UnixStream;

pub(crate) fn path_offset(sockaddr: &libc::sockaddr_un) -> usize {
    let base = sockaddr as *const _ as usize;
    let path = &sockaddr.sun_path as *const _ as usize;
    path - base
}
