use std::sync::atomic::{AtomicUsize, Ordering};

use lazy_static::lazy_static;

lazy_static! {
    // Global id generator
    static ref ID_GEN: AtomicUsize = AtomicUsize::new(1);
}

/// Used to generate thread id.
pub(crate) fn gen_id() -> usize {
    ID_GEN.fetch_add(1, Ordering::AcqRel)
}

pub(crate) fn get_current_thread_id() -> usize {
    crate::runtime::CURRENT.with(|ctx| ctx.thread_id)
}
