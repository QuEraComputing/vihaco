use vihaco_runtime::{component, composite, dialect};
#[dialect] mod arithmetic { #[instruction] pub struct Add(pub i64); }
#[component] #[instructions { arithmetic }] struct Cpu;
impl<I> vihaco_runtime::Execute<I> for Cpu {
    type Message=(); type Effect=(); type Error=std::convert::Infallible;
    fn execute(&mut self, _: &I, _:()) -> Result<(),Self::Error> { Ok(()) }
}
type Alias = Cpu;
pub use __vihaco_fields_Cpu as __vihaco_fields_Alias;
#[composite] struct Machine { #[device(1)] cpu: Alias }
fn main() {}
