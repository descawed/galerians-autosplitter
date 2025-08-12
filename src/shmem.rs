// TODO: Windows support
use std::ffi::OsStr;
use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use memmap2::Mmap;
use nix::sys::signal::kill;
use nix::unistd::Pid;
use num_traits::{ConstZero, FromBytes};

const SHMEM_PATH: &str = "/dev/shm";
const EMULATOR_STRINGS: [(&str, &str); 2] = [("duckstation_", "DuckStation"), ("pcsx-redux-wram-", "PCSX-Redux")];

const fn addr(address: u32) -> usize {
    (address & 0x1FFFFFF) as usize
}

fn is_pid_alive(pid: i32) -> bool {
    !matches!(kill(Pid::from_raw(pid), None), Err(nix::errno::Errno::ESRCH))
}

fn pid_from_path(path: &Path) -> Result<i32> {
    let file_name = path.file_name().map(OsStr::to_string_lossy).ok_or_else(|| anyhow!("Can't get PID from filename of {path:?} because this path isn't a file"))?;

    for (prefix, _) in &EMULATOR_STRINGS {
        if file_name.starts_with(prefix) {
            // check that the PID in the filename is still active, as I've seen at least
            // PCSX-redux leave shm objects around after the process exited
            let pid_str = file_name.strip_prefix(prefix).unwrap();
            if let Ok(pid) = pid_str.parse::<i32>() {
                return Ok(pid);
            }
        }
    }
    
    bail!("Did not find PID in file name {file_name}")
}

#[derive(Debug)]
pub struct GameMemory(Mmap, Option<i32>);

impl GameMemory {
    pub fn from_shmem(path: &Path, pid: Option<i32>) -> Result<Self> {
        // if we get a path from the discover method, we know it had a valid PID, so if we don't
        // find a valid PID, that means this path came directly from the user. in that case, we'll
        // assume they know what they're doing and we'll just go without the PID.
        let pid = pid.or_else(|| pid_from_path(path).ok());
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file) }?;
        Ok(Self(mmap, pid))
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

            for (prefix, emu_name) in &EMULATOR_STRINGS {
                if file_name.starts_with(prefix) {
                    // check that the PID in the filename is still active, as I've seen at least
                    // PCSX-redux leave shm objects around after the process exited
                    let pid_str = file_name.strip_prefix(prefix).unwrap();
                    match pid_str.parse::<i32>() {
                        Ok(pid) => {
                            if is_pid_alive(pid) {
                                // this one looks good; we'll use this
                                log::info!("Found {emu_name} shared memory {file_name}");
                                return Self::from_shmem(&path, Some(pid)).map(Some);
                            }
                        }
                        Err(_) => {
                            log::warn!("Found shm object that looked like {emu_name} but couldn't parse the PID: {file_name}");
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
    
    /// Check whether the emulator process providing this memory is still alive
    pub fn check_pulse(&self) -> bool {
        match self.1 {
            Some(pid) => is_pid_alive(pid),
            // if we don't have a PID, we don't know whether the emulator process is still alive or not,
            // so we'll just continue in blissful ignorance
            None => true,
        }
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

    pub fn read_nums<const M: usize, const N: usize, T: FromBytes<Bytes = [u8; N]> + ConstZero>(&self, address: u32) -> [T; M] {
        let mut out = [T::ZERO; M];
        let bytes = self.read_slice(address, M * N);
        for (num, bytes) in out.iter_mut().zip(bytes.chunks_exact(N)) {
            let mut num_bytes = [0u8; N];
            num_bytes.copy_from_slice(bytes);
            *num = T::from_le_bytes(&num_bytes);
        }

        out
    }
}