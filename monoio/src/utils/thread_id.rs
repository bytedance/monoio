use std::sync::{
    atomic::{AtomicUsize, Ordering},
    LazyLock,
};

// thread id begins from 16.
// 0 is default thread
// 1-15 are unused
static ID_GEN: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(16));

pub(crate) const DEFAULT_THREAD_ID: usize = 0;

/// Used to generate thread id.
pub(crate) fn gen_id() -> usize {
    ID_GEN.fetch_add(1, Ordering::AcqRel)
}

pub(crate) fn get_current_thread_id() -> usize {
    crate::runtime::CURRENT.with(|ctx| ctx.thread_id)
}

pub(crate) fn try_get_current_thread_id() -> Option<usize> {
    crate::runtime::CURRENT.try_with(|maybe_ctx| maybe_ctx.map(|ctx| ctx.thread_id))
}
