use vihaco_runtime::{dialect, Lower, Syntax};

#[dialect]
mod invalid {
    #[instruction]
    #[pattern = "'call $0:u64"]
    pub struct Call(pub u32);
}

fn main() {
    // u64 already supports TryInto<u32>, but lowering must still be authored.
    let _ = invalid::Call::<Syntax>(7).lower(&mut ());
}
