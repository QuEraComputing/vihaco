use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] pub struct Halt { x: u32 } }

fn main() {}
