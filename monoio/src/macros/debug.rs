#[cfg(all(debug_assertions, feature = "debug"))]
macro_rules! debug_eprintln {
    ($( $args:expr ),*) => { eprintln!( $( $args ),* ); }
}

#[cfg(not(all(debug_assertions, feature = "debug")))]
macro_rules! debug_eprintln {
    ($( $args:expr ),*) => {};
}
