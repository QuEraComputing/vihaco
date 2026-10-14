use vihaco_runtime::dialect;

#[dialect]
mod invalid {
    #[instruction]
    #[pattern = "'call $0:(u32 u64)"]
    pub struct Call(pub u32);
}

fn main() {}
