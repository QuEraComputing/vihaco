struct Cpu<T>(T);
impl<T> vm::Component for Cpu<T> {}
impl<T: Copy, I: vm::InstructionSpec> vm::Execute<I> for Cpu<T> {
    type Message = (); type Effect = (); type Error = vm::eyre::Report;
    fn execute(&mut self, _: &I, _: ()) -> Result<(), Self::Error> { Ok(()) }
}
fn check<T>() { vm::require_coverage::<Cpu<T>, external::control::__VihacoDialect>(); }
fn main() {}
