use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction(extra)] pub struct Halt; }

fn main() {}
