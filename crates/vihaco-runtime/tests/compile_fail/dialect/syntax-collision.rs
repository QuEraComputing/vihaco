use vihaco_runtime::dialect;

#[dialect]
mod invalid {
    pub mod syntax {}
    #[instruction]
    pub struct Halt;
}

fn main() {}
