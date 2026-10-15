struct Cpu;
impl vm::Component for Cpu {}
impl vm::Execute<external::control::Halt> for Cpu {
    type Message = (); type Effect = (); type Error = vm::eyre::Report;
    fn execute(&mut self, _: &external::control::Halt, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
struct AddedDialect;
impl vm::DialectSpec for AddedDialect {
    const LEAVES: &'static [vm::Leaf] = &[];
}
impl<C: vm::Execute<external::control::Halt> + vm::Execute<external::control::Omitted>> vm::Coverage<C> for AddedDialect {}
fn check() { vm::require_coverage::<Cpu, AddedDialect>(); }
fn main() {}
