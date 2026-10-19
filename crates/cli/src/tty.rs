//! The controlling terminal, opened as `/dev/tty`, for what only the person
//! at the keyboard may enter: a recovery phrase, and the answer to a
//! confirmation. Neither is read from argv, environment, a file, or stdin,
//! so neither lands in shell history, a process list, or a transcript, and
//! a program driving stdin cannot answer for the user.
//!
//! Limit: a harness that gives the agent a terminal of its own (a pty) can
//! type into it. There the harness's permission rules are the guard.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};

use rustix::termios::{self, LocalModes, OptionalActions, Termios};
use zeroize::Zeroizing;

/// Longer input is refused rather than reallocated, which would leave a
/// copy of a secret behind in freed memory.
const MAX_INPUT: usize = 1024;
const CTRL_C: u8 = 0x03;
const CTRL_D: u8 = 0x04;

#[derive(Debug, thiserror::Error)]
pub enum TtyError {
    #[error("this needs a terminal: it reads only from the keyboard, never from a pipe or a file")]
    NoTerminal,
    #[error("cancelled")]
    Cancelled,
    #[error("input longer than {MAX_INPUT} bytes")]
    TooLong,
    #[error("input is not UTF-8")]
    NotText,
    #[error("terminal: {0}")]
    Io(#[from] io::Error),
}

pub struct Tty(File);

impl Tty {
    pub fn open() -> Result<Self, TtyError> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .map(Self)
            .map_err(|_| TtyError::NoTerminal)
    }

    pub fn say(&mut self, text: &str) -> Result<(), TtyError> {
        self.0.write_all(text.as_bytes())?;
        Ok(self.0.flush()?)
    }

    /// A line typed with echo on.
    pub fn ask(&mut self, prompt: &str) -> Result<Zeroizing<String>, TtyError> {
        self.say(prompt)?;
        read_line(&self.0)
    }

    /// A line typed with echo off. Ctrl-C arrives as a byte rather than a
    /// signal, so the terminal is always restored.
    pub fn ask_secret(&mut self, prompt: &str) -> Result<Zeroizing<String>, TtyError> {
        self.say(prompt)?;
        let line = {
            let _restore = Quiet::new(&self.0)?;
            read_line(&self.0)
        };
        self.say("\n")?;
        line
    }
}

/// Byte by byte, unbuffered, into a buffer that never grows.
fn read_line(mut tty: &File) -> Result<Zeroizing<String>, TtyError> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(MAX_INPUT));
    let mut byte = [0u8; 1];
    loop {
        match tty.read(&mut byte)? {
            0 => return Err(TtyError::Cancelled),
            _ => match byte[0] {
                b'\n' | b'\r' => break,
                CTRL_C | CTRL_D => return Err(TtyError::Cancelled),
                _ if bytes.len() == MAX_INPUT => return Err(TtyError::TooLong),
                b => bytes.push(b),
            },
        }
    }
    let text = String::from_utf8(std::mem::take(&mut *bytes)).map_err(|e| {
        drop(Zeroizing::new(e.into_bytes()));
        TtyError::NotText
    })?;
    Ok(Zeroizing::new(text))
}

/// Echo and signal keys off until dropped.
struct Quiet<'a> {
    tty: &'a File,
    saved: Termios,
}

impl<'a> Quiet<'a> {
    fn new(tty: &'a File) -> Result<Self, TtyError> {
        let saved = termios::tcgetattr(tty).map_err(io::Error::from)?;
        let mut quiet = saved.clone();
        quiet
            .local_modes
            .remove(LocalModes::ECHO | LocalModes::ECHONL | LocalModes::ISIG);
        // Flush, as getpass does: anything typed before the prompt is
        // discarded rather than taken as the secret.
        termios::tcsetattr(tty, OptionalActions::Flush, &quiet).map_err(io::Error::from)?;
        Ok(Self { tty, saved })
    }
}

impl Drop for Quiet<'_> {
    fn drop(&mut self) {
        // Nothing to do if restoring fails: the terminal is going away.
        let _ = termios::tcsetattr(self.tty, OptionalActions::Now, &self.saved);
    }
}
