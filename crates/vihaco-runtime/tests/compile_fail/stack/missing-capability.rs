use vihaco_runtime::{ReusableMessageRecipe, Component, Composite, Execute, Field, PopFromStack};

struct Instruction;
struct Device;
impl Component for Device {}
impl Execute<Instruction> for Device {
    type Message = PopFromStack<()>;
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &Instruction, _: Self::Message) -> Result<(), Self::Error> { Ok(()) }
}

struct Route;
struct Machine(Device);
impl Composite for Machine {}
impl Field<Route> for Machine {
    type Child = Device;
    fn get(&self) -> &Device { &self.0 }
    fn get_mut(&mut self) -> &mut Device { &mut self.0 }
    const SOURCE: &'static [&'static str] = &["device"];
}

fn main() {
    let _ = <PopFromStack<()> as ReusableMessageRecipe<Machine, Route, Instruction, ()>>::prepare(&mut Machine(Device), &Instruction, &());
}
