#[declarations::dialect]
mod foo { #[instruction] #[cfg(any())] pub struct A; }
fn main() {}
