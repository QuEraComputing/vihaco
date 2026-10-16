use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] #[pattern = "'a $1"] pub struct A(pub u32); }

fn main() {}
