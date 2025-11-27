#[cfg(feature = "unstable")]
use std::sync::LazyLock;
use std::{
    sync::{Arc, Mutex},
    task::Waker,
};

use crossbeam_queue::SegQueue;
#[cfg(not(feature = "unstable"))]
use once_cell::sync::Lazy as LazyLock;
use rustc_hash::FxHashMap;

use crate::driver::UnparkHandle;

static UNPARK: LazyLock<Mutex<FxHashMap<usize, UnparkHandle>>> =
    LazyLock::new(|| Mutex::new(FxHashMap::default()));

// Global waker sender map
static WAKER_QUEUE: LazyLock<Mutex<FxHashMap<usize, Arc<SegQueue<Waker>>>>> =
    LazyLock::new(|| Mutex::new(FxHashMap::default()));

macro_rules! lock {
    ($x: ident) => {
        $x.lock()
            .expect("Unable to lock global map, which is unexpected")
    };
}

pub(crate) fn register_unpark_handle(id: usize, unpark: UnparkHandle) {
    lock!(UNPARK).insert(id, unpark);
}

pub(crate) fn unregister_unpark_handle(id: usize) {
    lock!(UNPARK).remove(&id);
}

pub(crate) fn get_unpark_handle(id: usize) -> Option<UnparkHandle> {
    lock!(UNPARK).get(&id).cloned()
}

pub(crate) fn register_waker_queue(id: usize, sender: Arc<SegQueue<Waker>>) {
    lock!(WAKER_QUEUE).insert(id, sender);
}

pub(crate) fn unregister_waker_queue(id: usize) {
    lock!(WAKER_QUEUE).remove(&id);
}

pub(crate) fn get_waker_queue(id: usize) -> Option<Arc<SegQueue<Waker>>> {
    lock!(WAKER_QUEUE).get(&id).cloned()
}
