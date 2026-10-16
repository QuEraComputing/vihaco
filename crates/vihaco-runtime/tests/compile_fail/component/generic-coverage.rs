#[vihaco_runtime::dialect]
mod control {
    #[instruction]
    pub struct Halt;
    #[instruction]
    pub struct NewlyAdded;
}
#[vihaco_runtime::component]
#[instructions { control }]
struct Cpu<T>(T);
impl vihaco_runtime::Execute<control::Halt> for Cpu<u8> {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
impl vihaco_runtime::Execute<control::NewlyAdded> for Cpu<u8> {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::NewlyAdded, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
fn main() {}
