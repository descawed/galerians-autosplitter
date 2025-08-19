use std::cell::{RefCell, Ref};
use std::fmt::Debug;
use std::path::Path;
use std::ptr;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use num_traits::{ConstZero, FromBytes};
use sysinfo::{Pid, Process, ProcessesToUpdate, ProcessRefreshKind, RefreshKind, System, UpdateKind};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::UnixSharedMemoryClient as PlatformSharedMemoryClient;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::WindowsSharedMemoryClient as PlatformSharedMemoryClient;

const EMULATOR_MAX_RAM: usize = 0x800000;

#[derive(Debug)]
pub struct Platform {
    system: System,
    last_refresh: Instant,
    refresh_interval: Duration,
}

impl Platform {
    fn process_refresh_kind() -> ProcessRefreshKind {
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet)
    }

    fn refresh_kind() -> RefreshKind {
        RefreshKind::nothing().with_processes(Self::process_refresh_kind())
    }

    pub fn new(refresh_interval: Duration) -> Self {
        let system = System::new_with_specifics(Self::refresh_kind());
        Self {
            system,
            last_refresh: Instant::now(),
            refresh_interval,
        }
    }

    pub fn refresh(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            Self::process_refresh_kind(),
        );
        self.last_refresh = Instant::now();
    }

    pub fn refresh_if_stale(&mut self) {
        if (Instant::now() - self.last_refresh) >= self.refresh_interval {
            self.refresh();
        }
    }

    pub fn is_pid_alive(&self, pid: Pid) -> bool {
        self.system.process(pid).is_some_and(Process::exists)
    }

    pub fn active_processes(&self) -> impl Iterator<Item = (Pid, &Process)> {
        self.system.processes().iter().map(|(pid, process)| (*pid, process))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum EmulatorType {
    DuckStation,
    PcsxRedux,
}

impl EmulatorType {
    const fn all() -> [Self; 2] {
        [Self::DuckStation, Self::PcsxRedux]
    }

    const fn prefix(&self) -> &'static str {
        match self {
            Self::DuckStation => "duckstation_",
            Self::PcsxRedux => "pcsx-redux-wram-",
        }
    }

    const fn name(&self) -> &'static str {
        match self {
            Self::DuckStation => "DuckStation",
            Self::PcsxRedux => "PCSX-Redux",
        }
    }

    const fn exe_substring(&self) -> &'static str {
        match self {
            Self::DuckStation => "duckstation",
            Self::PcsxRedux => "pcsx-redux",
        }
    }
}

#[derive(Debug, Clone)]
struct EmulatorProcess {
    emulator_type: EmulatorType,
    pid: Pid,
    platform: Rc<RefCell<Platform>>,
}

impl EmulatorProcess {
    const fn new(emulator_type: EmulatorType, pid: Pid, platform: Rc<RefCell<Platform>>) -> Self {
        Self { emulator_type, pid, platform }
    }

    fn is_alive(&self) -> bool {
        self.platform.acquire().is_pid_alive(self.pid)
    }

    fn shmem_name(&self) -> String {
        format!("{}{}", self.emulator_type.prefix(), self.pid.as_u32())
    }
}

pub trait PlatformInterface {
    fn acquire(&self) -> Ref<'_, Platform>;

    fn search_for_emulator(self: &Rc<Self>) -> Option<Emulator>;
}

impl PlatformInterface for RefCell<Platform> {
    fn acquire(&self) -> Ref<'_, Platform> {
        // if we fail to mutably borrow the platform, we just ignore it; it's not the end of the
        // world if the data's a little stale
        if let Ok(mut platform) = self.try_borrow_mut() {
            platform.refresh_if_stale();
        }
        // if we fail to immutably borrow the platform, that's a problem
        self.borrow()
    }

    fn search_for_emulator(self: &Rc<Self>) -> Option<Emulator> {
        let platform = self.acquire();
        for (pid, process) in platform.active_processes() {
            let Some(exe_name) = process.exe().and_then(Path::file_name) else {
                continue;
            };

            let lc_exe_name = exe_name.to_string_lossy().to_lowercase();

            for emulator_type in EmulatorType::all() {
                if !lc_exe_name.contains(emulator_type.exe_substring()) {
                    continue;
                }

                let emulator_process = EmulatorProcess::new(
                    emulator_type,
                    pid,
                    Rc::clone(self),
                );

                match Emulator::from_process(emulator_process) {
                    Ok(emulator) => return Some(emulator),
                    Err(e) => log::warn!("Failed to attach to {} process {}: {}", emulator_type.name(), pid, e),
                }
            }
        }

        None
    }
}

trait SharedMemoryClient: Debug {
    fn open(name: &str, size: usize) -> Result<Self> where Self: Sized;

    fn base(&self) -> *const u8;

    fn size(&self) -> usize;

    fn end(&self) -> *const u8 {
        // SAFETY: if creation of the mapping succeeded in the open method, then self.base() will be
        // the base address of an allocation of at least self.size() bytes, and the size will not
        // exceed isize::MAX
        unsafe { self.base().byte_add(self.size()) }
    }
}

#[derive(Debug)]
pub struct Emulator {
    shared_memory: PlatformSharedMemoryClient,
    process: EmulatorProcess,
}

impl Emulator {
    fn from_process(process: EmulatorProcess) -> Result<Self> {
        let shared_memory = PlatformSharedMemoryClient::open(&process.shmem_name(), EMULATOR_MAX_RAM)?;

        Ok(Self {
            shared_memory,
            process,
        })
    }
    
    /// Check whether the emulator process providing this memory is still alive
    pub fn check_pulse(&self) -> bool {
        self.process.is_alive()
    }

    fn address_to_pointer(&self, address: u32) -> *const u8 {
        let offset = (address & 0x1FFFFFF) as usize;
        if offset >= self.shared_memory.size() {
            panic!("Attempted to read from an address beyond the end of emulated RAM: address {address:08X}");
        }

        let base = self.shared_memory.base();
        // SAFETY: we've checked above that offset is within the size of the allocation
        unsafe { base.byte_add(offset) }
    }

    fn pointer_for_range(&self, address: u32, size: usize) -> *const u8 {
        let data_start = self.address_to_pointer(address);
        let memory_end = self.shared_memory.end();
        // SAFETY: address_to_pointer guarantees that the returned pointer will be within the
        // allocation and less than the end pointer
        let bytes_available = unsafe { memory_end.byte_offset_from_unsigned(data_start) };
        if size > bytes_available {
            panic!("Attempted to read a number of bytes that would pass the end of emulated RAM: address {address:08X}, size {size}");
        }

        data_start
    }

    pub fn read<const N: usize>(&self, address: u32) -> [u8; N] {
        let mut buf = [0u8; N];
        self.read_into(address, &mut buf);
        buf
    }

    pub fn read_into(&self, address: u32, buf: &mut [u8]) {
        let size = buf.len();
        let src = self.pointer_for_range(address, size);
        let dest = buf.as_mut_ptr();
        // SAFETY: pointer_for_range guarantees that it's safe to copy at least `size` bytes from
        // the returned pointer. there's no way the provided buffer slice could overlap with the
        // source data without additional unsafe abuse of the shared memory object outside of this
        // function.
        unsafe {
            ptr::copy_nonoverlapping(src, dest, size);
        }
    }

    pub fn read_num<const N: usize, T: FromBytes<Bytes = [u8; N]>>(&self, address: u32) -> T {
        let bytes: T::Bytes = self.read(address);
        T::from_le_bytes(&bytes)
    }

    pub fn read_nums<const M: usize, const N: usize, T: FromBytes<Bytes = [u8; N]> + ConstZero>(&self, address: u32) -> [T; M] {
        let mut out = [T::ZERO; M];

        // what we would really like to do here is create a [u8; { M * N }] array, but
        // unfortunately, this kind of use of const generics is not yet stabilized. instead, we'll
        // create an array that's sized to hold a whole number of any numeric primitive and then
        // just loop if we need to. the size in bytes of all the primitive integers and floats is a
        // power of 2, and the largest are i128 and u128 at 16 bytes. let's go with 256 bytes for
        // 16 128-bit integers and progressively more smaller ones.
        let mut buf = [0u8; 256];
        let buf_size = buf.len();
        // just double-check that our assumptions are valid
        if buf_size % N != 0 {
            panic!("Buffer of size {} cannot hold a whole number of {}-sized elements", buf_size, N);
        }

        let buf_elements = buf_size / N;
        let size = M * N;
        let mut bytes_remaining = size;
        let mut src = self.pointer_for_range(address, size);
        let dest = buf.as_mut_ptr();
        let mut i = 0usize;
        while i < M {
            let bytes_to_read = buf_size.min(bytes_remaining);
            let end = (i + buf_elements).min(M);

            // SAFETY: pointer_for_range guarantees that it's safe to copy at least `size` bytes
            // from the returned pointer. it's impossible that the ranges could overlap as the
            // destination buffer is a local stack variable that was just created.
            unsafe {
                ptr::copy_nonoverlapping(src, dest, bytes_to_read);
            }

            for (num, bytes) in (&mut out[i..end]).iter_mut().zip(buf.chunks_exact(N)) {
                let mut bytes_for_num = [0u8; N];
                bytes_for_num.copy_from_slice(bytes);
                *num = T::from_le_bytes(&bytes_for_num);
            }

            i += buf_elements;
            bytes_remaining -= bytes_to_read;
            // SAFETY: we keep track of the number of bytes remaining in the region that we
            // validated we could read from to ensure we don't go past the end.
            src = unsafe { src.byte_add(bytes_to_read) };
        }

        out
    }
}