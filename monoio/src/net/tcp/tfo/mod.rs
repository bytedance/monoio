//! TCP Fast Open

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod macos;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use macos::{set_tcp_fastopen, set_tcp_fastopen_force_enable};

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) use linux::{set_tcp_fastopen, try_set_tcp_fastopen_connect};
