#[vihaco_runtime::dialect]
mod control {
    #[instruction]
    pub struct Halt;
    #[instruction]
    pub struct NewlyAdded;
}
#[vihaco_runtime::dialect]
mod foreign { #[instruction] pub struct Extra; }
#[vihaco_runtime::component]
#[instructions { control }]
struct Cpu;
impl vihaco_runtime::Execute<control::Halt> for Cpu {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::Halt, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
impl vihaco_runtime::Execute<control::NewlyAdded> for Cpu {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &control::NewlyAdded, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
impl vihaco_runtime::Execute<foreign::Extra> for Cpu {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &foreign::Extra, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
fn check() { vihaco_runtime::__private::require_available::<Cpu, foreign::Extra>(); }
fn main() {}
