use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] #[instruction] pub struct Halt; }

fn main() {}
