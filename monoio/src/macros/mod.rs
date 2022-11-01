//! Useful macros.

#[macro_use]
pub mod scoped_tls;

#[macro_use]
mod pin;

#[macro_use]
mod ready;

#[macro_use]
mod select;

#[macro_use]
mod join;

#[macro_use]
mod try_join;

// Includes re-exports needed to implement macros
#[doc(hidden)]
pub mod support;

#[macro_use]
mod debug;
