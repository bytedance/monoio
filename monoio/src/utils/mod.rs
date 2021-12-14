//! Common utils

pub(crate) mod linked_list;

mod rand;
pub use rand::thread_rng_n;

pub mod slab;

#[cfg(feature = "sync")]
pub(crate) mod thread_id;

#[cfg(feature = "utils")]
mod bind_to_cpu_set;
#[cfg(feature = "utils")]
pub use bind_to_cpu_set::{bind_to_cpu_set, BindError};
