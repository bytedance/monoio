//! IO traits

mod async_read_rent;
mod async_read_rent_ext;
mod async_write_rent;
mod async_write_rent_ext;

mod async_buf_read;

pub use async_read_rent::{AsyncReadRent, AsyncReadRentAt};
pub use async_read_rent_ext::AsyncReadRentExt;
pub use async_write_rent::{AsyncWriteRent, AsyncWriteRentAt};
pub use async_write_rent_ext::AsyncWriteRentExt;

pub use async_buf_read::{AsyncBufRead, BufReader};

mod util;
pub use util::copy;
