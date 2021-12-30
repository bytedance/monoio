#![allow(unreachable_pub)]
//! TCP related.

mod listener;
mod split;
mod stream;

pub use listener::TcpListener;
pub use split::{
    OwnedReadHalf as TcpOwnedReadHalf, OwnedWriteHalf as TcpOwnedWriteHalf,
    ReadHalf as TcpReadHalf, ReuniteError as TcpReuniteError, WriteHalf as TcpWriteHalf,
};
pub use stream::TcpStream;
