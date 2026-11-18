//! Where the text comes from: the arguments, or stdin (requirements §3).

use std::io::{self, IsTerminal, Read};
use std::sync::mpsc;
use std::time::Duration;

use crate::error::CliError;

/// The text to translate. No `Debug`: it is user text.
pub struct Input {
    pub text: String,
    /// Text was given as arguments and stdin held data too (the arguments won).
    pub ignored_stdin: bool,
}

/// How long to wait to find out whether stdin holds data when the text was
/// also given as arguments. Only matters for a pipe that stays open and idle.
const STDIN_PEEK_TIMEOUT: Duration = Duration::from_millis(200);

/// Reads the input.
///
/// - Text arguments win. If stdin is not a terminal and holds data, that is
///   reported through `ignored_stdin` so the caller can warn.
/// - Without arguments, stdin is read when it is not a terminal.
/// - With neither, the text is empty (the caller prints the help).
///
/// `max_chars` only bounds how much is read from stdin, so a huge file cannot
/// fill memory. The exact length check is done by the caller.
pub fn read_input(args_text: Option<String>, max_chars: usize) -> Result<Input, CliError> {
    let stdin_is_terminal = io::stdin().is_terminal();
    match args_text {
        Some(text) => Ok(Input {
            text,
            ignored_stdin: !stdin_is_terminal && stdin_has_data(),
        }),
        None if stdin_is_terminal => Ok(Input {
            text: String::new(),
            ignored_stdin: false,
        }),
        None => Ok(Input {
            text: read_stdin(max_chars)?,
            ignored_stdin: false,
        }),
    }
}

/// Reads all of stdin, but never more than `max_chars` characters' worth of bytes.
fn read_stdin(max_chars: usize) -> Result<String, CliError> {
    // One character is at most 4 bytes in UTF-8.
    let max_bytes = max_chars.saturating_mul(4);
    let mut bytes = Vec::new();
    // Read one byte more than allowed, to tell "exactly at the limit" from "over".
    let read_limit = u64::try_from(max_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    io::stdin()
        .lock()
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|_| CliError::Unexpected("cannot read standard input"))?;
    if bytes.len() > max_bytes {
        return Err(CliError::TooLarge { limit: max_chars });
    }
    String::from_utf8(bytes).map_err(|_| CliError::Usage("standard input is not valid UTF-8"))
}

/// True if stdin has at least one byte before EOF.
///
/// A plain `read` would hang forever on a pipe that is open but silent, so it
/// runs on another thread and we give up after a short wait. The thread is
/// left behind when we give up; the process exits soon after anyway.
fn stdin_has_data() -> bool {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut byte = [0u8; 1];
        let has_data = matches!(io::stdin().read(&mut byte), Ok(count) if count > 0);
        // The receiver may be gone already; that is fine.
        let _ = sender.send(has_data);
    });
    receiver.recv_timeout(STDIN_PEEK_TIMEOUT).unwrap_or(false)
}
