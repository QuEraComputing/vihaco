use eyre::Result;
use vihaco::{Effects, observe};

#[derive(Debug, Clone)]
pub struct Line(pub String);

/// A standalone observer: it reacts to delivered effects and owns no
/// instructions or messages of its own.
#[derive(Debug, Default)]
pub struct Collector {
    lines: Vec<String>,
}

// `#[observe(T)]` generates `Observe<T>`; the handler is named
// `observe_<snake_case_effect>` and may return follow-up effects.
#[observe(Line)]
impl Collector {
    fn observe_line(&mut self, effect: &Line) -> Result<Effects<()>> {
        self.lines.push(effect.0.clone());
        Ok(Effects::none())
    }
}
