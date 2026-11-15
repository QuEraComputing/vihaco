use vihaco_runtime::{component, composite, dialect, Verify};

trait OperandDepth {
    fn push(&mut self);
}

#[dialect]
mod control {
    #[instruction]
    #[pattern = "'push"]
    pub struct Push;
}
impl<C: OperandDepth + ?Sized> Verify<C> for control::Push {
    fn verify(&self, cx: &mut C) -> vihaco_runtime::eyre::Result<()> {
        cx.push();
        Ok(())
    }
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
/// A verifier context that does not provide `OperandDepth`.
struct Verifier;
fn main() {
    let instruction = __vihaco_MachineInstruction::V_cpu__control__Push(control::Push);
    let _ = instruction.verify(&mut Verifier);
}
