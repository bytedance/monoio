use std::{cell::RefCell, collections::HashSet, rc::Rc};

use crate::driver::op::OpCanceller;

/// CancelHandle is used to pass to io actions with CancelableAsyncReadRent.
/// Create a CancelHandle with Canceller::handle.
#[derive(Clone)]
pub struct CancelHandle {
    shared: Rc<RefCell<Shared>>,
}

/// Canceller is a user-hold struct to cancel io operations.
/// A canceller can assocate with multiple io operations.
#[derive(Default)]
pub struct Canceller {
    shared: Rc<RefCell<Shared>>,
}

pub(crate) struct AssocateGuard {
    op_canceller: OpCanceller,
    shared: Rc<RefCell<Shared>>,
}

#[derive(Default)]
struct Shared {
    canceled: bool,
    slot_ref: HashSet<OpCanceller>,
}

impl Canceller {
    /// Create a new Canceller.
    pub fn new() -> Self {
        Default::default()
    }

    /// Cancel all related operations.
    pub fn cancel(self) -> Self {
        let mut slot = HashSet::new();
        {
            let mut shared = self.shared.borrow_mut();
            shared.canceled = true;
            std::mem::swap(&mut slot, &mut shared.slot_ref);
        }

        for op_canceller in slot.iter() {
            unsafe { op_canceller.cancel() };
        }
        slot.clear();
        Canceller {
            shared: Rc::new(RefCell::new(Shared {
                canceled: false,
                slot_ref: slot,
            })),
        }
    }

    /// Create a CancelHandle which can be used to pass to io operation.
    pub fn handle(&self) -> CancelHandle {
        CancelHandle {
            shared: self.shared.clone(),
        }
    }
}

impl CancelHandle {
    pub(crate) fn canceled(&self) -> bool {
        self.shared.borrow().canceled
    }

    pub(crate) fn assocate_op(self, op_canceller: OpCanceller) -> AssocateGuard {
        {
            let mut shared = self.shared.borrow_mut();
            shared.slot_ref.insert(op_canceller.clone());
        }
        AssocateGuard {
            op_canceller,
            shared: self.shared,
        }
    }
}

impl Drop for AssocateGuard {
    fn drop(&mut self) {
        let mut shared = self.shared.borrow_mut();
        shared.slot_ref.remove(&self.op_canceller);
    }
}

pub(crate) fn operation_canceled() -> std::io::Error {
    std::io::Error::from_raw_os_error(125)
}
