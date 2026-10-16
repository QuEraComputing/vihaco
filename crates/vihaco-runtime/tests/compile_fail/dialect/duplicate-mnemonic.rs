use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] pub struct Halt; #[instruction] #[pattern = "'halt $0"] pub struct Other(pub u32); }

fn main() {}
