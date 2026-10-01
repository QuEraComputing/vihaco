use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] #[pattern = "'halt"] pub struct r#type; }

fn main() {}
