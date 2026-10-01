use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] #[pattern = "'same"] pub struct A; #[instruction] #[pattern = "'same"] pub struct B; }

fn main() {}
