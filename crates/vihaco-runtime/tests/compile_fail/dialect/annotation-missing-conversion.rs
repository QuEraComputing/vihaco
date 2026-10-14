use vihaco_runtime::dialect;

#[dialect]
mod invalid {
    #[instruction]
    #[pattern = "'call $0:f64"]
    pub struct Call(pub u32);
}

fn main() {}
