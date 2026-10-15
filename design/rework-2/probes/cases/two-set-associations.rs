include!("local-machine.rs");
#[declarations::instruction_set(composite = Local)]
enum First { #[target(cpu)] Halt(external::control::Halt) }
#[declarations::instruction_set(composite = Local)]
enum Second { #[target(cpu)] Halt(external::control::Halt) }
fn main() {}
