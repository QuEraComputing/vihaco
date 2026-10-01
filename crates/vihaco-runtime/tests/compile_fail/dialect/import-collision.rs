use vihaco_runtime::dialect;

#[dialect]
mod invalid { use super::Operand as __HaltSurface; #[instruction] pub struct Halt; }
pub struct Operand;

fn main() {}
