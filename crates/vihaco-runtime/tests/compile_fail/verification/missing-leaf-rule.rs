use vihaco_runtime::{component, composite, dialect, Verify};

#[dialect]
mod control {
    #[instruction]
    #[pattern = "'nop"]
    pub struct Nop;

    #[instruction]
    #[pattern = "'unverified"]
    pub struct Unverified;
}
impl<C: ?Sized> Verify<C> for control::Nop {
    fn verify(&self, _: &mut C) -> vihaco_runtime::eyre::Result<()> { Ok(()) }
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
    let instruction = __vihaco_MachineInstruction::V_cpu__control__Nop(control::Nop);
    let _ = instruction.verify(&mut ());
}
