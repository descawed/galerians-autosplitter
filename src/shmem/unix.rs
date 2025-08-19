use std::ffi::{CString, c_void};
use std::io::Error;
use std::ptr;

use anyhow::{bail, Result};

use super::SharedMemoryClient;

unsafe fn close_shm(name: &str, fd: libc::c_int) {
    if fd == -1 {
        // don't try to close an invalid file descriptor
        return;
    }

    let status = unsafe { libc::close(fd) };
    if status == -1 {
        let errno = Error::last_os_error();
        log::error!("Failed to close shared memory object {name} with fd {fd}: {errno}");
    }
    // we can't really do anything about a failure to close, so don't bother trying to return an
    // error to the caller.
}

#[derive(Debug)]
pub(super) struct UnixSharedMemoryClient {
    name: String,
    shm_fd: libc::c_int,
    base: *mut c_void,
    size: usize,
}

impl SharedMemoryClient for UnixSharedMemoryClient {
    fn open(name: &str, size: usize) -> Result<Self> {
        let c_name = CString::new(name)?;
        let shm_fd = unsafe {
            libc::shm_open(c_name.as_ptr(), libc::O_RDONLY, 0)
        };
        if shm_fd == -1 {
            let errno = Error::last_os_error();
            bail!("Failed to open shared memory object {name}: {errno}");
        }

        let base = unsafe {
            libc::mmap(ptr::null_mut(), size, libc::PROT_READ, libc::MAP_SHARED, shm_fd, 0)
        };
        if base == libc::MAP_FAILED {
            let errno = Error::last_os_error();
            unsafe { close_shm(name, shm_fd) };
            bail!("Failed to map shared memory object {name}: {errno}");
        }

        Ok(Self {
            name: String::from(name),
            shm_fd,
            base,
            size,
        })
    }

    fn base(&self) -> *const u8 {
        self.base as *const u8
    }

    fn size(&self) -> usize {
        self.size
    }
}

impl Drop for UnixSharedMemoryClient {
    fn drop(&mut self) {
        // SAFETY: we don't complete construction of a value to be dropped unless the mapping
        // succeeded
        let status = unsafe { libc::munmap(self.base, self.size) };
        if status == -1 {
            let errno = Error::last_os_error();
            log::error!("Failed to unmap shared memory {} at {:p}: {}", self.name, self.base, errno);
        }

        unsafe {
            close_shm(&self.name, self.shm_fd);
        }
    }
}