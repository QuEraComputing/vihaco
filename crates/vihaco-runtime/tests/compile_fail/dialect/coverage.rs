use vihaco_runtime::dialect;

#[dialect]
mod invalid { #[instruction] pub struct A; #[instruction] pub struct B; }
struct Cpu;
impl vihaco_runtime::Component for Cpu {}
impl vihaco_runtime::Execute<invalid::A> for Cpu {
 type Message = (); type Effect = (); type Error = std::convert::Infallible;
 fn execute(&mut self, _: &invalid::A, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
fn check() { vihaco_runtime::__private::require_coverage::<Cpu, <invalid::A as vihaco_runtime::__private::InstructionSpec>::Dialect>(); }

fn main() {}
