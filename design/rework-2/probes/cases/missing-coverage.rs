struct Cpu;
impl vm::Component for Cpu {}
fn check() { vm::require_coverage::<Cpu, external::control::__VihacoDialect>(); }
fn main() {}
