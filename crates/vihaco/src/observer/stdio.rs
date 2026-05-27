use std::io::Write;

use crate::observe;
use eyre::Result;

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

#[observe(StdoutEffect)]
impl StdoutObserver {
    fn observe_stdout_effect(&mut self, effect: &StdoutEffect) -> Result<crate::Effects<()>> {
        self.write_stdout(&effect.0)?;
        Ok(crate::Effects::none())
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
