#![allow(unreachable_pub)]
//! TCP related.

mod listener;
mod split;
mod stream;
mod tfo;

pub use listener::TcpListener;
pub use split::{TcpOwnedReadHalf, TcpOwnedWriteHalf};
pub use stream::{TcpConnectOpts, TcpStream};

#[cfg(feature = "poll-io")]
pub mod stream_poll;
