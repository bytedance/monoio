use std::sync::{
    atomic::{AtomicUsize, Ordering},
    LazyLock,
};

// thread id begins from 16.
// 0 is always reserved
// 1 is blocking thread
// 2-15 are unused
static ID_GEN: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(16));

pub(crate) const BLOCKING_THREAD_ID: usize = 1;

/// Used to generate thread id.
pub(crate) fn gen_id() -> usize {
    ID_GEN.fetch_add(1, Ordering::AcqRel)
}

pub(crate) fn get_current_thread_id() -> usize {
    crate::runtime::CURRENT.with(|ctx| ctx.thread_id)
}
