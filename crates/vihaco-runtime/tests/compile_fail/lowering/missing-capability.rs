use vihaco_runtime::{component, composite, dialect, Lower, Syntax};

trait Lookup { fn lookup(&self, value: u64) -> u32; }
#[dialect]
mod control {
    #[instruction]
    #[pattern = "'call $0:u64"]
    pub struct Call(pub u32);
}
impl<C: Lookup + ?Sized> Lower<C> for control::Call<Syntax> {
    type Runtime = control::Call;
    fn lower(self, context: &mut C) -> eyre::Result<Self::Runtime> {
        Ok(control::Call(context.lookup(self.0)))
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
struct Loader;
impl vihaco_runtime::loader::LoadOwnSstSection<()> for Loader {
    fn load_own_sst_section<'bc>(
        &mut self, _: vihaco_runtime::SstSectionView<'bc, ()>,
    ) -> eyre::Result<()> { Ok(()) }
}
fn main() {
    use vihaco_runtime::loader::LoadOwnSstSection;
    let source = __vihaco_MachineInstruction::<Syntax>::V_cpu__control__Call(control::Call(7));
    let _ = Loader.lower_instructions([source]);
}
