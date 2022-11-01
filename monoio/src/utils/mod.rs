//! Common utils

pub(crate) mod linked_list;
pub(crate) mod slab;
pub(crate) mod thread_id;
pub(crate) mod uring_detect;

mod rand;
pub use rand::thread_rng_n;
pub use uring_detect::detect_uring;

#[cfg(feature = "utils")]
mod bind_to_cpu_set;
#[cfg(feature = "utils")]
pub use bind_to_cpu_set::{bind_to_cpu_set, BindError};
