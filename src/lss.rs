use std::fmt::Display;
use std::io::{BufRead, BufReader, Error as IoError, ErrorKind, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};

const MAX_RETRIES: u8 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimerPhase {
    NotRunning,
    Running,
    Ended,
    Paused,
}

impl TimerPhase {
    fn try_from_raw(s: &[u8]) -> Option<TimerPhase> {
        match s {
            b"NotRunning" => Some(TimerPhase::NotRunning),
            b"Running" => Some(TimerPhase::Running),
            b"Ended" => Some(TimerPhase::Ended),
            b"Paused" => Some(TimerPhase::Paused),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct LiveSplit {
    addr: SocketAddr,
    connection: BufReader<TcpStream>,
    is_connected: bool,
}

impl LiveSplit {
    pub fn create(port: u16) -> Result<Self> {
        let addr = ("localhost", port).to_socket_addrs()?.next().unwrap();
        let connection = BufReader::new(TcpStream::connect(addr)?);
        log::info!("Successfully connected to LiveSplit");

        Ok(Self {
            addr,
            connection,
            is_connected: true,
        })
    }

    pub fn try_reconnect(&mut self) -> Result<()> {
        self.connection = BufReader::new(TcpStream::connect(self.addr)?);
        self.is_connected = true;
        log::info!("LiveSplit connection re-established");
        Ok(())
    }

    pub const fn is_connected(&self) -> bool {
        self.is_connected
    }

    fn connection_lost<T: Display>(&mut self, error: &T) {
        log::error!("LiveSplit connection lost: {error}");
        self.is_connected = false;
        // doesn't matter if the shutdown fails as the connection appears to be hosed anyway
        let _ = self.connection.get_mut().shutdown(Shutdown::Both);
    }

    fn handle_error(&mut self, error: &IoError) {
        if matches!(error.kind(),
            ErrorKind::BrokenPipe | ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset
            | ErrorKind::HostUnreachable | ErrorKind::NetworkDown | ErrorKind::NetworkUnreachable
            | ErrorKind::NotConnected
        ) {
            // the connection is lost. flag it as such and return the error.
            self.connection_lost(&error);
        }
    }

    pub fn send(&mut self, data: &[u8]) -> Result<()> {
        for _ in 0..MAX_RETRIES {
            match self.connection.get_mut().write_all(data) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    self.handle_error(&e);
                    if !self.is_connected {
                        // the error was unrecoverable; bail
                        return Err(e.into());
                    }

                    log::warn!("LiveSplit communication error: {e}. Retrying...");
                    thread::sleep(RETRY_DELAY);
                }
            }
        }

        self.connection_lost(&"Maximum retries exceeded");
        bail!("Maximum retries exceeded");
    }

    pub fn recv(&mut self) -> Result<Vec<u8>> {
        for _ in 0..MAX_RETRIES {
            let mut buf = Vec::new();
            match self.connection.read_until(b'\n', &mut buf) {
                Ok(_) => {
                    // strip the trailing newline
                    buf.pop();
                    // strip any trailing carriage return
                    if buf.last() == Some(&b'\r') {
                        buf.pop();
                    }
                    return Ok(buf);
                }
                Err(e) => {
                    self.handle_error(&e);
                    if !self.is_connected {
                        // the error was unrecoverable; bail
                        return Err(e.into());
                    }

                    log::warn!("LiveSplit communication error: {e}. Retrying...");
                    thread::sleep(RETRY_DELAY);
                }
            }
        }

        self.connection_lost(&"Maximum retries exceeded");
        bail!("Maximum retries exceeded");
    }

    pub fn recv_int(&mut self) -> Result<i64> {
        let raw = self.recv()?;
        Ok(str::from_utf8(&raw)?.parse()?)
    }

    pub fn split(&mut self) -> Result<()> {
        self.send(b"startorsplit\n")
    }

    pub fn reset(&mut self) -> Result<()> {
        self.send(b"reset\n")
    }

    pub fn get_split_index(&mut self) -> Result<i64> {
        self.send(b"getsplitindex\n")?;
        self.recv_int()
    }

    pub fn get_timer_phase(&mut self) -> Result<TimerPhase> {
        self.send(b"gettimerphase\n")?;
        let response = self.recv()?;
        TimerPhase::try_from_raw(&response).ok_or_else(|| anyhow!("Invalid timer phase received from LiveSplit server"))
    }

    pub fn get_custom_variable_value(&mut self, variable_name: &str) -> Result<Option<String>> {
        self.send(b"getcustomvariablevalue ")?;
        self.send(variable_name.as_bytes())?;
        self.send(b"\n")?;

        let response = self.recv()?;
        let value = str::from_utf8(&response)?;
        if value == "-" || value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value.to_string()))
        }
    }
}