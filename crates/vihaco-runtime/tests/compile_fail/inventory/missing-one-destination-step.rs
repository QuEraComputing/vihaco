use vihaco_runtime::{component, composite, dialect};
#[dialect] mod arithmetic { #[instruction] pub struct Add(pub i64); }
#[component] #[instructions { arithmetic }] struct Cpu;
impl<I> vihaco_runtime::Execute<I> for Cpu {
    type Message=(); type Effect=(); type Error=std::convert::Infallible;
    fn execute(&mut self, _: &I, _:()) -> Result<(),Self::Error> { Ok(()) }
}
#[composite] struct Machine { #[device(1)] first: Cpu, #[device(2)] second: Cpu }
impl vihaco_runtime::Step<vihaco_runtime::field!(Machine::first), arithmetic::Add> for Machine {
    fn step(&mut self, _: &arithmetic::Add, _: &()) -> vihaco_runtime::eyre::Result<()> { Ok(()) }
}
fn main() {
    fn require_dispatch<T: vihaco_runtime::Dispatch>() {}
    require_dispatch::<Machine>();
}
