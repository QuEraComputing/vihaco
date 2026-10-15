#[declarations::dialect]
mod foo {
    #[instruction]
    pub struct A;
    #[instruction]
    pub struct B;
    mod nested { pub struct Helper; }
}
fn main() {}
