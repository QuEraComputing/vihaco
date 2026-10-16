use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] pub struct Halt; mod inner { #[instruction] pub struct Nested; } }

fn main() {}
