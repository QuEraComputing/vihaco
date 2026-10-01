use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] #[pattern = "bad"] pub struct Bad; #[instruction] pub struct Halt; #[instruction] #[pattern = "'halt"] pub struct Other; }

fn main() {}
