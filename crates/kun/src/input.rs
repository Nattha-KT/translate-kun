//! Where the text comes from: the arguments, or stdin (requirements §3).

use std::io::{self, IsTerminal, Read};

use crate::error::CliError;

/// The text to translate. No `Debug`: it is user text.
pub struct Input {
    pub text: String,
    /// Text was given as arguments while stdin is not a terminal (it was
    /// piped or redirected). Stdin is not read; the caller prints a warning.
    pub ignored_stdin: bool,
}

/// Reads the input.
///
/// - Text arguments win, and stdin is never read in that case. If stdin is
///   not a terminal, `ignored_stdin` is set so the caller can warn. This is
///   decided by what stdin *is*, not by what it holds: checking for data
///   would need a timing guess that a slow producer defeats.
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
            ignored_stdin: !stdin_is_terminal && !stdin_is_known_empty(),
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

/// True when stdin is `/dev/null` or an empty regular file, so there is
/// nothing to ignore and no warning is needed. This only looks at file
/// metadata: it never reads and never waits.
#[cfg(unix)]
fn stdin_is_known_empty() -> bool {
    use std::os::fd::AsFd;

    let Ok(fd) = io::stdin().as_fd().try_clone_to_owned() else {
        return false;
    };
    match std::fs::File::from(fd).metadata() {
        Ok(metadata) => is_known_empty(&metadata),
        Err(_) => false,
    }
}

/// Off Unix, stdin is always treated as possibly holding data, so the warning
/// shows whenever stdin is redirected. On Windows std cannot tell a pipe from
/// an empty file without `unsafe` (a pipe reports `is_file()` and `len() == 0`
/// there), and guessing wrong would silently drop piped text.
#[cfg(not(unix))]
fn stdin_is_known_empty() -> bool {
    false
}

/// An empty regular file, or the `/dev/null` device.
#[cfg(unix)]
fn is_known_empty(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};

    if metadata.is_file() {
        return metadata.len() == 0;
    }
    if metadata.file_type().is_char_device() {
        return match std::fs::metadata("/dev/null") {
            Ok(null) => null.rdev() == metadata.rdev(),
            Err(_) => false,
        };
    }
    false
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn dev_null_and_empty_files_count_as_empty() {
        let null = std::fs::File::open("/dev/null").unwrap();
        assert!(is_known_empty(&null.metadata().unwrap()));

        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty");
        std::fs::write(&empty, "").unwrap();
        assert!(is_known_empty(&std::fs::metadata(&empty).unwrap()));
    }

    #[test]
    fn files_with_content_do_not_count_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let full = dir.path().join("full");
        std::fs::write(&full, "x").unwrap();
        assert!(!is_known_empty(&std::fs::metadata(&full).unwrap()));
    }
}
