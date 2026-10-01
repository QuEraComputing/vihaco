use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] pub enum Halt { A } }

fn main() {}
