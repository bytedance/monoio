#[cfg(all(debug_assertions, feature = "debug"))]
macro_rules! trace {
    ($( $args:expr ),*) => { tracing::trace!( $( $args ),* ); }
}

#[cfg(not(all(debug_assertions, feature = "debug")))]
macro_rules! trace {
    ($( $args:expr ),*) => {};
}

#[allow(unused_macros)]
#[cfg(all(debug_assertions, feature = "debug"))]
macro_rules! info {
    ($( $args:expr ),*) => { tracing::info!( $( $args ),* ); }
}

#[allow(unused_macros)]
#[cfg(not(all(debug_assertions, feature = "debug")))]
macro_rules! info {
    ($( $args:expr ),*) => {};
}
