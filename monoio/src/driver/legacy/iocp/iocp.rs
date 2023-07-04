use std::{
    os::windows::prelude::{AsRawHandle, FromRawHandle, IntoRawHandle, RawHandle},
    time::Duration,
};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::IO::{
        CreateIoCompletionPort, GetQueuedCompletionStatusEx, PostQueuedCompletionStatus,
        OVERLAPPED_ENTRY,
    },
};

#[derive(Debug)]
pub struct CompletionPort {
    handle: HANDLE,
}

impl CompletionPort {
    pub fn new(value: u32) -> std::io::Result<Self> {
        let handle = unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, 0, 0, value) };

        if handle == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Self { handle })
        }
    }

    pub fn add_handle(&self, token: usize, handle: HANDLE) -> std::io::Result<()> {
        let result = unsafe { CreateIoCompletionPort(handle, self.handle, token, 0) };

        if result == 0 {
            return Err(std::io::Error::last_os_error());
        } else {
            Ok(())
        }
    }

    pub fn get_many<'a>(
        &self,
        entries: &'a mut [OVERLAPPED_ENTRY],
        timeout: Option<Duration>,
    ) -> std::io::Result<&'a mut [OVERLAPPED_ENTRY]> {
        let mut count = 0;
        let result = unsafe {
            GetQueuedCompletionStatusEx(
                self.handle,
                entries.as_mut_ptr(),
                std::cmp::min(entries.len(), u32::max_value() as usize) as u32,
                &mut count,
                duration_millis(timeout),
                0,
            )
        };

        if result == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(&mut entries[..count as usize])
        }
    }

    pub fn post(&self, entry: OVERLAPPED_ENTRY) -> std::io::Result<()> {
        let result = unsafe {
            PostQueuedCompletionStatus(
                self.handle,
                entry.dwNumberOfBytesTransferred,
                entry.lpCompletionKey,
                entry.lpOverlapped,
            )
        };

        if result == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for CompletionPort {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

impl AsRawHandle for CompletionPort {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle as RawHandle
    }
}

impl FromRawHandle for CompletionPort {
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self {
            handle: handle as HANDLE,
        }
    }
}

impl IntoRawHandle for CompletionPort {
    fn into_raw_handle(self) -> RawHandle {
        self.handle as RawHandle
    }
}

#[inline]
fn duration_millis(dur: Option<Duration>) -> u32 {
    if let Some(dur) = dur {
        // `Duration::as_millis` truncates, so round up. This avoids
        // turning sub-millisecond timeouts into a zero timeout, unless
        // the caller explicitly requests that by specifying a zero
        // timeout.
        let dur_ms = dur
            .checked_add(Duration::from_nanos(999_999))
            .unwrap_or(dur)
            .as_millis();
        std::cmp::min(dur_ms, u32::MAX as u128) as u32
    } else {
        u32::MAX
    }
}
