// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::io::Write;

use eyre::Result;
use vihaco_runtime::{Effects, Observe};

#[derive(Debug, Clone)]
pub struct StdoutEffect(pub String);

#[derive(Debug, Default)]
pub struct StdoutObserver {
    output: std::io::Cursor<Vec<u8>>,
}

impl StdoutObserver {
    pub fn write_stdout(&mut self, text: &str) -> Result<()> {
        self.output.write_all(text.as_bytes())?;
        Ok(())
    }

    pub fn output(&self) -> &[u8] {
        self.output.get_ref()
    }
}

impl Observe<StdoutEffect> for StdoutObserver {
    type Effect = ();
    type Error = eyre::Report;

    fn observe(&mut self, effect: &StdoutEffect) -> Result<Effects<()>> {
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
