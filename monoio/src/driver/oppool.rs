use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

// pub(crate) mod close;

mod accept;
// mod connect;
// mod fsync;
// mod open;
// mod poll;
// mod read;
// mod recv;
// mod send;
// mod write;

#[cfg(all(target_os = "linux", feature = "splice"))]
// mod splice;
