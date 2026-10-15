include!("local-machine.rs");
trait Config { type Machine; type Instruction; }
struct Settings;
impl Config for Settings {
    type Machine = Local;
    type Instruction = external::control::Halt;
}
#[declarations::instruction_set(composite = <Settings as Config>::Machine,)]
pub enum Selected {
    #[target(cpu)] Halt(<Settings as Config>::Instruction),
}
fn main() {}
