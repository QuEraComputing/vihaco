use vihaco_runtime::{component, composite, dialect};
#[dialect] mod arithmetic { #[instruction] pub struct Add(pub i64); }
#[component] #[instructions { arithmetic }] struct Cpu<T>(T);
impl<T,I> vihaco_runtime::Execute<I> for Cpu<T> {
    type Message=(); type Effect=(); type Error=std::convert::Infallible;
    fn execute(&mut self, _: &I, _:()) -> Result<(),Self::Error> { Ok(()) }
}
#[composite] struct Machine<T> { #[device(1)] cpu: Cpu<T> }
impl vihaco_runtime::Step<vihaco_runtime::field!(Machine<u8>::cpu), arithmetic::Add> for Machine<u8> {
    fn step(&mut self, _: &arithmetic::Add, _: &()) -> vihaco_runtime::eyre::Result<()> { Ok(()) }
}
fn main() {
    fn require_dispatch<T: vihaco_runtime::Dispatch>() {}
    require_dispatch::<Machine<u16>>();
}
