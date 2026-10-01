use vihaco_runtime::dialect;

#[dialect]
mod invalid { pub struct __HaltSurface; #[instruction] pub struct Halt; }

fn main() {}
