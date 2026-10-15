#[declarations::dialect]
mod foo {
    #[instruction] pub struct A;
    mod nested { #[instruction] pub struct B; }
}
fn main() {}
