use vihaco_runtime::dialect;

#[dialect]
mod invalid {
    #[instruction(parser = custom)]
    pub struct Halt;
}

fn main() {}
