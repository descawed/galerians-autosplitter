use anyhow::{bail, Result};
use windows::core::{PCWSTR, HSTRING};
use windows::Win32::Foundation::{HANDLE, CloseHandle, GetLastError};
use windows::Win32::System::Memory::{FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, OpenFileMappingW, MapViewOfFile, UnmapViewOfFile};

use super::SharedMemoryClient;

unsafe fn close_handle(name: &str, handle: HANDLE) {
    if let Err(e) = unsafe { CloseHandle(handle) } {
        log::error!("Failed to close shared memory mapping {name}: {e}");
    }
}

#[derive(Debug)]
pub(super) struct WindowsSharedMemoryClient {
    name: String,
    handle: HANDLE,
    base: MEMORY_MAPPED_VIEW_ADDRESS,
    size: usize,
}

impl SharedMemoryClient for WindowsSharedMemoryClient {
    fn open(name: &str, size: usize) -> Result<Self> {
        let wide_name = HSTRING::from(name);
        let p_name = PCWSTR(wide_name.as_ptr());
        let handle = match unsafe { OpenFileMappingW(FILE_MAP_READ.0, false, p_name) } {
            Ok(handle) => handle,
            Err(e) => bail!("Failed to open shared memory mapping {name}: {e}"),
        };

        let base = unsafe {
            MapViewOfFile(handle, FILE_MAP_READ, 0, 0, size)
        };
        if base.Value.is_null() {
            let error = unsafe { GetLastError() };
            unsafe { close_handle(name, handle) };
            let hresult = error.to_hresult();
            bail!("Failed to map shared memory {}: {} ({:08X})", name, hresult.message(), error.0);
        }

        Ok(Self {
            name: String::from(name),
            handle,
            base,
            size,
        })
    }

    fn base(&self) -> *const u8 {
        self.base.Value as *const u8
    }

    fn size(&self) -> usize {
        self.size
    }
}

impl Drop for WindowsSharedMemoryClient {
    fn drop(&mut self) {
        // SAFETY: we don't complete construction of a value to be dropped unless the mapping
        // completed successfully.
        if let Err(e) = unsafe { UnmapViewOfFile(self.base) } {
            log::error!("Failed to unmap shared memory {} at address {:p}: {}", self.name, self.base.Value, e);
        }

        unsafe {
            close_handle(&self.name, self.handle);
        }
    }
}