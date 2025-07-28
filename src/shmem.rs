// TODO: Windows support
use std::ffi::OsStr;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::Result;
use memmap2::Mmap;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use num_traits::FromBytes;

const SHMEM_PATH: &str = "/dev/shm";
const EMULATOR_PREFIXES: [&str; 2] = ["duckstation_", "pcsx-redux-wram-"];

const fn addr(address: u32) -> usize {
    (address & 0x1FFFFFF) as usize
}

fn is_pid_alive(pid: i32) -> bool {
    match kill(Pid::from_raw(pid), None) {
        Ok(()) => true,
        Err(nix::errno::Errno::ESRCH) => false,
        Err(_) => true,
    }
}

#[derive(Debug)]
pub struct GameMemory(Mmap);

impl GameMemory {
    pub fn from_shmem(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file) }?;
        Ok(Self(mmap))
    }

    pub fn discover() -> Result<Option<Self>> {
        let shmem_path = PathBuf::from(SHMEM_PATH);
        for entry in shmem_path.read_dir()? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(file_name) = path.file_name().map(OsStr::to_string_lossy) else {
                continue;
            };

            for prefix in &EMULATOR_PREFIXES {
                if file_name.starts_with(prefix) {
                    // check that the PID in the filename is still active, as I've seen at least
                    // PCSX-redux leave shm objects around after the process exited
                    let pid_str = file_name.strip_prefix(prefix).unwrap();
                    match pid_str.parse::<i32>() {
                        Ok(pid) => {
                            if is_pid_alive(pid) {
                                // this one looks good; we'll use this
                                log::info!("Found emulator shared memory {file_name}");
                                return Self::from_shmem(&path).map(Some);
                            }
                        }
                        Err(_) => {
                            log::warn!("Found shm object that looked right but couldn't parse the PID: {file_name}");
                            continue;
                        }
                    }
                }
            }
        }

        // we didn't find any emulator memory. maybe the emulator hasn't been started yet; we'll check
        // again later
        Ok(None)
    }

    pub fn read<const N: usize>(&self, address: u32) -> [u8; N] {
        let a = addr(address);
        let mut buf = [0u8; N];
        buf.copy_from_slice(&self.0[a..a + N]);
        buf
    }

    pub fn read_slice(&self, address: u32, size: usize) -> &[u8] {
        let a = addr(address);
        &self.0[a..a + size]
    }

    pub fn read_num<const N: usize, T: FromBytes<Bytes = [u8; N]>>(&self, address: u32) -> T {
        let bytes: T::Bytes = self.read(address);
        T::from_le_bytes(&bytes)
    }
}