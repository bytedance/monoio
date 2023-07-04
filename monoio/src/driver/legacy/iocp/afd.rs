use std::{
    ffi::c_void,
    fs::File,
    os::windows::prelude::{AsRawHandle, FromRawHandle, RawHandle},
    sync::atomic::{AtomicUsize, Ordering},
};

use windows_sys::Win32::{
    Foundation::{
        RtlNtStatusToDosError, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS, STATUS_NOT_FOUND,
        STATUS_PENDING, STATUS_SUCCESS, UNICODE_STRING,
    },
    Storage::FileSystem::{
        NtCreateFile, SetFileCompletionNotificationModes, FILE_OPEN, FILE_SHARE_READ,
        FILE_SHARE_WRITE, SYNCHRONIZE,
    },
    System::WindowsProgramming::{
        NtDeviceIoControlFile, FILE_SKIP_SET_EVENT_ON_HANDLE, IO_STATUS_BLOCK, IO_STATUS_BLOCK_0,
        OBJECT_ATTRIBUTES,
    },
};

use super::CompletionPort;

#[link(name = "ntdll")]
extern "system" {
    /// See <https://processhacker.sourceforge.io/doc/ntioapi_8h.html#a0d4d550cad4d62d75b76961e25f6550c>
    ///
    /// This is an undocumented API and as such not part of <https://github.com/microsoft/win32metadata>
    /// from which `windows-sys` is generated, and also unlikely to be added, so
    /// we manually declare it here
    fn NtCancelIoFileEx(
        FileHandle: HANDLE,
        IoRequestToCancel: *mut IO_STATUS_BLOCK,
        IoStatusBlock: *mut IO_STATUS_BLOCK,
    ) -> NTSTATUS;
}

static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(0);

macro_rules! s {
    ($($id:expr)+) => {
        &[$($id as u16),+]
    }
}

pub const POLL_RECEIVE: u32 = 0b0_0000_0001;
pub const POLL_RECEIVE_EXPEDITED: u32 = 0b0_0000_0010;
pub const POLL_SEND: u32 = 0b0_0000_0100;
pub const POLL_DISCONNECT: u32 = 0b0_0000_1000;
pub const POLL_ABORT: u32 = 0b0_0001_0000;
pub const POLL_LOCAL_CLOSE: u32 = 0b0_0010_0000;
// Not used as it indicated in each event where a connection is connected, not
// just the first time a connection is established.
// Also see https://github.com/piscisaureus/wepoll/commit/8b7b340610f88af3d83f40fb728e7b850b090ece.
pub const POLL_CONNECT: u32 = 0b0_0100_0000;
pub const POLL_ACCEPT: u32 = 0b0_1000_0000;
pub const POLL_CONNECT_FAIL: u32 = 0b1_0000_0000;

pub const KNOWN_EVENTS: u32 = POLL_RECEIVE
    | POLL_RECEIVE_EXPEDITED
    | POLL_SEND
    | POLL_DISCONNECT
    | POLL_ABORT
    | POLL_LOCAL_CLOSE
    | POLL_ACCEPT
    | POLL_CONNECT_FAIL;

#[repr(C)]
pub struct AfdPollHandleInfo {
    pub handle: HANDLE,
    pub events: u32,
    pub status: NTSTATUS,
}

#[repr(C)]
pub struct AfdPollInfo {
    pub timeout: i64,
    pub number_of_handles: u32,
    pub exclusive: u32,
    pub handles: [AfdPollHandleInfo; 1],
}

pub struct Afd {
    file: File,
}

impl Afd {
    pub fn new(cp: &CompletionPort) -> std::io::Result<Self> {
        const AFD_NAME: &[u16] = s!['\\' 'D' 'e' 'v' 'i' 'c' 'e' '\\' 'A' 'f' 'd' '\\' 'I' 'o'];
        let mut device_name = UNICODE_STRING {
            Length: std::mem::size_of_val(AFD_NAME) as u16,
            MaximumLength: std::mem::size_of_val(AFD_NAME) as u16,
            Buffer: AFD_NAME.as_ptr() as *mut u16,
        };
        let mut device_attributes = OBJECT_ATTRIBUTES {
            Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: 0,
            ObjectName: &mut device_name,
            Attributes: 0,
            SecurityDescriptor: std::ptr::null_mut(),
            SecurityQualityOfService: std::ptr::null_mut(),
        };
        let mut handle = INVALID_HANDLE_VALUE;
        let mut iosb = unsafe { std::mem::zeroed::<IO_STATUS_BLOCK>() };
        let result = unsafe {
            NtCreateFile(
                &mut handle,
                SYNCHRONIZE,
                &mut device_attributes,
                &mut iosb,
                std::ptr::null_mut(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                FILE_OPEN,
                0,
                std::ptr::null_mut(),
                0,
            )
        };

        if result != STATUS_SUCCESS {
            let error = unsafe { RtlNtStatusToDosError(result) };
            return Err(std::io::Error::from_raw_os_error(error as i32));
        }

        let file = unsafe { File::from_raw_handle(handle as RawHandle) };
        // Increment by 2 to reserve space for other types of handles.
        // Non-AFD types (currently only NamedPipe), use odd numbered
        // tokens. This allows the selector to differentiate between them
        // and dispatch events accordingly.
        let token = NEXT_TOKEN.fetch_add(2, Ordering::Relaxed) + 2;
        cp.add_handle(token, file.as_raw_handle() as HANDLE)?;
        let result = unsafe {
            SetFileCompletionNotificationModes(
                handle,
                FILE_SKIP_SET_EVENT_ON_HANDLE as u8, // This is just 2, so fits in u8
            )
        };

        if result == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Self { file })
        }
    }

    pub unsafe fn poll(
        &self,
        info: &mut AfdPollInfo,
        iosb: *mut IO_STATUS_BLOCK,
        overlapped: *mut c_void,
    ) -> std::io::Result<bool> {
        const IOCTL_AFD_POLL: u32 = 0x00012024;
        let info_ptr = info as *mut _ as *mut c_void;
        (*iosb).Anonymous.Status = STATUS_PENDING;

        let result = NtDeviceIoControlFile(
            self.file.as_raw_handle() as HANDLE,
            0,
            None,
            overlapped,
            iosb,
            IOCTL_AFD_POLL,
            info_ptr,
            std::mem::size_of::<AfdPollInfo>() as u32,
            info_ptr,
            std::mem::size_of::<AfdPollInfo>() as u32,
        );

        match result {
            STATUS_SUCCESS => Ok(true),
            STATUS_PENDING => Ok(false),
            status => {
                let error = RtlNtStatusToDosError(status);
                Err(std::io::Error::from_raw_os_error(error as i32))
            }
        }
    }

    pub unsafe fn cancel(&self, iosb: *mut IO_STATUS_BLOCK) -> std::io::Result<()> {
        if (*iosb).Anonymous.Status != STATUS_PENDING {
            return Ok(());
        }
        let mut cancel_iosb = IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        };
        let status = NtCancelIoFileEx(self.file.as_raw_handle() as HANDLE, iosb, &mut cancel_iosb);

        if status == STATUS_SUCCESS || status == STATUS_NOT_FOUND {
            Ok(())
        } else {
            let error = RtlNtStatusToDosError(status);
            Err(std::io::Error::from_raw_os_error(error as i32))
        }
    }
}
