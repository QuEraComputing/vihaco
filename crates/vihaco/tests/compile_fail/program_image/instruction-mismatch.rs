use vihaco::{Composite, Dispatch, ExecutionState, HasProgram, Image, Loadable};
use vihaco::module::LocalModule;

struct Machine(ExecutionState);
impl Composite for Machine {}
impl HasProgram for Machine {
    type Program = ExecutionState;
    fn program(&self) -> &ExecutionState { &self.0 }
    fn program_mut(&mut self) -> &mut ExecutionState { &mut self.0 }
}
impl Dispatch<LocalModule<u64, vihaco::Value, vihaco::Type>> for Machine {
    type Instruction = u32;
    fn dispatch(&mut self, _: &u32, _: &LocalModule<u64, vihaco::Value, vihaco::Type>) -> eyre::Result<()> {
        Ok(())
    }
}

fn main() {
    let image = Image::new(LocalModule::<u64, vihaco::Value, vihaco::Type>::default());
    let mut loaded = Loadable::new(image, Machine(ExecutionState::new()));
    loaded.execute(&[]).unwrap();
}
