use vihaco_runtime::{
    BuildMessage, Component, Composite, Execute, Field, HasOperandStack, PopFromStack, PopStack,
};

struct Instruction;
struct Device;
impl Component for Device {}
impl Execute<Instruction> for Device {
    type Message = PopFromStack<(f64,)>;
    type Effect = ();
    type Error = std::convert::Infallible;
    fn execute(&mut self, _: &Instruction, _: Self::Message) -> Result<(), Self::Error> { Ok(()) }
}

struct WordStorage;
impl PopStack<u64> for WordStorage {
    fn pop(&mut self) -> eyre::Result<u64> { Ok(0) }
}

struct Route;
struct Machine(WordStorage, Device);
impl Composite for Machine {}
impl Field<Route> for Machine {
    type Child = Device;
    fn get(&self) -> &Device { &self.1 }
    fn get_mut(&mut self) -> &mut Device { &mut self.1 }
    const SOURCE: &'static [&'static str] = &["device"];
}
impl HasOperandStack<Instruction> for Machine {
    type Source = WordStorage;
    fn operand_stack(&mut self, _: usize) -> &mut WordStorage { &mut self.0 }
}

fn main() {
    let _ = <Machine as BuildMessage<Route, Instruction>>::build_message(
        &mut Machine(WordStorage, Device), &Instruction, &());
}
