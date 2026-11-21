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

/// True when stdin is `/dev/null` (Unix only) or an empty regular file, so
/// there is nothing to ignore and no warning is needed. This only looks at
/// file metadata: it never reads and never waits.
#[cfg(unix)]
fn stdin_is_known_empty() -> bool {
    use std::os::fd::AsFd;

    let Ok(fd) = io::stdin().as_fd().try_clone_to_owned() else {
        return false;
    };
    stdin_file_is_known_empty(std::fs::File::from(fd))
}

/// Same check on Windows, through the stdin handle. The `NUL` device cannot
/// be recognised without `unsafe` (std cannot read its metadata), so `NUL`
/// still counts as "possibly holds data" and gets the warning.
#[cfg(windows)]
fn stdin_is_known_empty() -> bool {
    use std::os::windows::io::AsHandle;

    let Ok(handle) = io::stdin().as_handle().try_clone_to_owned() else {
        return false;
    };
    stdin_file_is_known_empty(std::fs::File::from(handle))
}

/// On other platforms, stdin is always treated as possibly holding data.
#[cfg(not(any(unix, windows)))]
fn stdin_is_known_empty() -> bool {
    false
}

#[cfg(any(unix, windows))]
fn stdin_file_is_known_empty(file: std::fs::File) -> bool {
    match file.metadata() {
        Ok(metadata) => is_known_empty(&metadata),
        Err(_) => false,
    }
}

/// An empty regular file, or the `/dev/null` device (Unix only).
#[cfg(any(unix, windows))]
fn is_known_empty(metadata: &std::fs::Metadata) -> bool {
    if metadata.is_file() {
        return metadata.len() == 0;
    }
    is_dev_null(metadata)
}

#[cfg(unix)]
fn is_dev_null(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};

    if !metadata.file_type().is_char_device() {
        return false;
    }
    match std::fs::metadata("/dev/null") {
        Ok(null) => null.rdev() == metadata.rdev(),
        Err(_) => false,
    }
}

#[cfg(windows)]
fn is_dev_null(_metadata: &std::fs::Metadata) -> bool {
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

#[cfg(all(test, any(unix, windows)))]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn dev_null_counts_as_empty() {
        let null = std::fs::File::open("/dev/null").unwrap();
        assert!(is_known_empty(&null.metadata().unwrap()));
    }

    #[test]
    fn empty_files_count_as_empty() {
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
