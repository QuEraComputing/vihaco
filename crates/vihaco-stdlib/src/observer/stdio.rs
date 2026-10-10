// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::fs::File;
use std::io::{self, Write};

use eyre::Result;
use vihaco_runtime::{Effects, observe};

#[derive(Debug, Clone)]
pub struct StdoutEffect(pub String);

/// Writes VM text to a buffer, process stdout, or an owned file.
///
/// The default destination is an empty buffer. Stdout and file destinations do
/// not retain a copy of the output. Writes do not add newlines. The observer
/// does not explicitly flush after writes; call [`Self::flush`] when needed.
#[derive(Debug)]
pub struct StdoutObserver {
    output: Output,
}

#[derive(Debug)]
enum Output {
    Buffer(Vec<u8>),
    Stdout(io::Stdout),
    File(File),
}

impl Default for StdoutObserver {
    fn default() -> Self {
        Self::buffered()
    }
}

impl StdoutObserver {
    /// Captures text in an empty buffer, as does [`Self::default`].
    pub fn buffered() -> Self {
        Self {
            output: Output::Buffer(Vec::new()),
        }
    }

    /// Writes text to process stdout without retaining a copy.
    pub fn stdout() -> Self {
        Self {
            output: Output::Stdout(io::stdout()),
        }
    }

    /// Takes ownership of an open file as the output destination.
    ///
    /// The caller controls the file's position and open options. Use
    /// [`File::create`] to create or truncate a file, or [`std::fs::OpenOptions`]
    /// to append. Write errors are reported when text is written.
    pub fn file(file: File) -> Self {
        Self {
            output: Output::File(file),
        }
    }

    /// Writes the text's UTF-8 bytes unchanged and propagates I/O errors.
    pub fn write_stdout(&mut self, text: &str) -> Result<()> {
        self.writer().write_all(text.as_bytes())?;
        Ok(())
    }

    /// Returns captured bytes, or an empty slice for stdout/file destinations.
    pub fn output(&self) -> &[u8] {
        match &self.output {
            Output::Buffer(buffer) => buffer,
            Output::Stdout(_) | Output::File(_) => &[],
        }
    }

    /// Flushes the destination and propagates I/O errors.
    ///
    /// This is a no-op for a memory buffer. For a file, this does not call
    /// [`File::sync_all`] or guarantee that the bytes have reached disk.
    pub fn flush(&mut self) -> Result<()> {
        self.writer().flush()?;
        Ok(())
    }

    fn writer(&mut self) -> &mut dyn Write {
        match &mut self.output {
            Output::Buffer(buffer) => buffer,
            Output::Stdout(stdout) => stdout,
            Output::File(file) => file,
        }
    }
}

#[observe(StdoutEffect)]
impl StdoutObserver {
    fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
        self.write_stdout(&effect.0)?;
        Ok(Effects::none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_stdout_appends_bytes() {
        let mut observer = StdoutObserver::default();

        observer.write_stdout("hello").unwrap();
        observer.write_stdout(" world").unwrap();

        assert_eq!(observer.output(), b"hello world");
    }
}
