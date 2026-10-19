use vihaco_runtime::{component, composite, dialect, Lower, Syntax};

#[dialect]
mod control {
    #[instruction]
    #[pattern = "'call $0:u64"]
    pub struct Call(pub u32);
}
#[component] #[instructions { control }] struct Cpu;
impl<I> vihaco_runtime::Execute<I> for Cpu {
    type Message = (); type Effect = (); type Error = std::convert::Infallible;
    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
#[composite] struct Machine { #[device(1)] cpu: Cpu }
impl<I> vihaco_runtime::Step<vihaco_runtime::field!(Machine::cpu), I> for Machine {
    fn step(&mut self, _: &I, _: &()) -> vihaco_runtime::eyre::Result<()> { Ok(()) }
}
fn main() {
    let source = __vihaco_MachineInstruction::<Syntax>::V_cpu__control__Call(control::Call(7));
    let _ = source.lower(&mut ());
}
