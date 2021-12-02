//! For compat with tokio AsyncRead and AsyncWrite.
mod tcp;

pub use tcp::TcpStreamCompat;
pub use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
