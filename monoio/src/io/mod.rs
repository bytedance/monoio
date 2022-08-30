//! IO traits

mod async_buf_read;
mod async_read_rent;
mod async_read_rent_ext;
mod async_write_rent;
mod async_write_rent_ext;

pub mod sink;
pub mod stream;

pub mod as_fd;
#[cfg(all(target_os = "linux", feature = "splice"))]
pub mod splice;

pub use async_buf_read::AsyncBufRead;
pub use async_read_rent::{AsyncReadRent, AsyncReadRentAt};
pub use async_read_rent_ext::AsyncReadRentExt;
pub use async_write_rent::{AsyncWriteRent, AsyncWriteRentAt};
pub use async_write_rent_ext::AsyncWriteRentExt;

mod util;
#[cfg(all(target_os = "linux", feature = "splice"))]
pub use util::zero_copy;
pub use util::{copy, BufReader, BufWriter, PrefixedReadIo};
