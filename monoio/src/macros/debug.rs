#[cfg(all(debug_assertions, feature = "debug"))]
macro_rules! tracing {
    ($( $args:expr ),*) => { tracing::trace!( $( $args ),* ); }
}

#[cfg(not(all(debug_assertions, feature = "debug")))]
macro_rules! tracing {
    ($( $args:expr ),*) => {};
}
