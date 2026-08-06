use eyre::Result;
use vihaco::{Effects, Observe};

#[derive(Debug, Clone)]
pub struct Line(pub String);

/// A standalone observer: it reacts to delivered effects and owns no
/// instructions or messages of its own.
#[derive(Debug, Default)]
pub struct Collector {
    lines: Vec<String>,
}

impl Observe<Line> for Collector {
    type Effect = ();
    type Error = eyre::Report;

    fn observe(&mut self, effect: &Line) -> Result<Effects<()>> {
        self.lines.push(effect.0.clone());
        Ok(Effects::none())
    }
}
