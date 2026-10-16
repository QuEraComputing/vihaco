use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] pub struct Halt; #[cfg_attr(all(), instruction)] pub struct Extra; }

fn main() {}
