use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] pub(crate) struct Halt; }

fn main() {}
