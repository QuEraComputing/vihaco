use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] pub struct Halt; #[instruction] pub struct Halt; }

fn main() {}
