#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Continue,
    Breakpoint,
    Return,
    Halt,
}
