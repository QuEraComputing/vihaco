use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] #[pattern = "'a"] #[pattern = "'b"] pub struct Halt; }

fn main() {}
