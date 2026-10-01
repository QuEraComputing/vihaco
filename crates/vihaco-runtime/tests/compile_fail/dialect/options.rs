use vihaco_runtime::dialect;

#[dialect(extra)]
mod invalid { #[instruction] pub struct Halt; }

fn main() {}
