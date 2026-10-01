use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] struct Halt; }

fn main() {}
