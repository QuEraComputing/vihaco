include!("local-machine.rs");
type Alias = chain::again::control::Halt;
#[declarations::instruction_set(composite = Local)]
enum Selected {
    #[target(cpu)] First(external::control::Halt),
    #[target(cpu)] Second(Alias),
}
fn main() {}
