use std::path::Path;

use crate::driver::op::Op;

pub(super) struct BuilderInner;

impl BuilderInner {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) async fn mkdir(&self, path: &Path) -> std::io::Result<()> {
        Op::mkdir(path)?.await.meta.result.map(|_| ())
    }
}
