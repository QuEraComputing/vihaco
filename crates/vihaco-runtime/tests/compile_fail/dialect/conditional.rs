use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] #[cfg(all())] pub struct Halt; }

fn main() {}
