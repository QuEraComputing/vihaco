#[vihaco_runtime::dialect]
mod control {
    #[instruction]
    pub struct Halt;
    #[instruction]
    pub struct NewlyAdded;
}
#[vihaco_runtime::component]
#[instructions { control }]
struct Cpu;
impl vihaco_runtime::Execute<control::Halt> for Cpu {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
fn main() {}
