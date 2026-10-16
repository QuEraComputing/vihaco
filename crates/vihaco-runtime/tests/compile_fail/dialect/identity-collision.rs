use vihaco_runtime::dialect;

#[dialect]
mod invalid { pub struct __VihacoDialect; #[instruction] pub struct Halt; }

fn main() {}
