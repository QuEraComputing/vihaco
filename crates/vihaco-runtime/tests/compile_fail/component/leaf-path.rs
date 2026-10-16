#[vihaco_runtime::dialect]
mod control {
    #[instruction]
    pub struct Halt;
    #[instruction]
    pub struct NewlyAdded;
}
#[vihaco_runtime::component]
#[instructions { control::Halt }]
struct Cpu;
fn main() {}
