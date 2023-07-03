use mio::Token;
use windows_sys::Win32::System::IO::OVERLAPPED_ENTRY;

use super::afd;

#[derive(Clone)]
pub struct Event {
    pub flags: u32,
    pub data: u64,
}

impl Event {
    pub fn new(token: Token) -> Event {
        Event {
            flags: 0,
            data: usize::from(token) as u64,
        }
    }

    pub fn token(&self) -> Token {
        Token(self.data as usize)
    }

    pub fn set_readable(&mut self) {
        self.flags |= afd::POLL_RECEIVE
    }

    pub fn set_writable(&mut self) {
        self.flags |= afd::POLL_SEND;
    }

    pub fn from_entry(status: &OVERLAPPED_ENTRY) -> Event {
        Event {
            flags: status.dwNumberOfBytesTransferred,
            data: status.lpCompletionKey as u64,
        }
    }

    pub fn to_entry(&self) -> OVERLAPPED_ENTRY {
        OVERLAPPED_ENTRY {
            dwNumberOfBytesTransferred: self.flags,
            lpCompletionKey: self.data as usize,
            lpOverlapped: std::ptr::null_mut(),
            Internal: 0,
        }
    }

    pub fn is_readable(&self) -> bool {
        self.flags & READABLE_FLAGS != 0
    }

    pub fn is_writable(&self) -> bool {
        self.flags & WRITABLE_FLAGS != 0
    }

    pub fn is_error(&self) -> bool {
        self.flags & ERROR_FLAGS != 0
    }

    pub fn is_read_closed(&self) -> bool {
        self.flags & READ_CLOSED_FLAGS != 0
    }

    pub fn is_write_closed(&self) -> bool {
        self.flags & WRITE_CLOSED_FLAGS != 0
    }

    pub fn is_priority(&self) -> bool {
        self.flags & afd::POLL_RECEIVE_EXPEDITED != 0
    }
}

pub(crate) const READABLE_FLAGS: u32 = afd::POLL_RECEIVE
    | afd::POLL_DISCONNECT
    | afd::POLL_ACCEPT
    | afd::POLL_ABORT
    | afd::POLL_CONNECT_FAIL;
pub(crate) const WRITABLE_FLAGS: u32 = afd::POLL_SEND | afd::POLL_ABORT | afd::POLL_CONNECT_FAIL;
pub(crate) const ERROR_FLAGS: u32 = afd::POLL_CONNECT_FAIL;
pub(crate) const READ_CLOSED_FLAGS: u32 =
    afd::POLL_DISCONNECT | afd::POLL_ABORT | afd::POLL_CONNECT_FAIL;
pub(crate) const WRITE_CLOSED_FLAGS: u32 = afd::POLL_ABORT | afd::POLL_CONNECT_FAIL;

pub struct Events {
    pub statuses: Box<[OVERLAPPED_ENTRY]>,

    pub events: Vec<Event>,
}

impl Events {
    pub fn with_capacity(cap: usize) -> Events {
        Events {
            statuses: unsafe { vec![std::mem::zeroed(); cap].into_boxed_slice() },
            events: Vec::with_capacity(cap),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.events.capacity()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn get(&self, idx: usize) -> Option<&Event> {
        self.events.get(idx)
    }

    pub fn clear(&mut self) {
        self.events.clear();
        for status in self.statuses.iter_mut() {
            *status = unsafe { std::mem::zeroed() };
        }
    }
}
