use std::path::Path;

use libc::mode_t;

use crate::driver::op::Op;

pub(super) struct BuilderInner {
    mode: libc::mode_t,
}

impl BuilderInner {
    pub(super) fn new() -> Self {
        Self { mode: 0o777 }
    }

    pub(super) async fn mkdir(&self, path: &Path) -> std::io::Result<()> {
        Op::mkdir(path, self.mode)?.await.meta.result.map(|_| ())
    }

    pub(super) fn set_mode(&mut self, mode: u32) {
        self.mode = mode as mode_t;
    }
}
