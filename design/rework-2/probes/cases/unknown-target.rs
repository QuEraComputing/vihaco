include!("local-machine.rs");
#[declarations::instruction_set(composite = Local)]
enum Selected { #[target(unknown)] Halt(external::control::Halt) }
fn main() {}
