#[declarations::dialect]
mod foo {
    #[instruction]
    pub struct A;
}
fn main() {
    let _: Option<<foo::A as vm::InstructionSpec>::Surface> = None;
}
