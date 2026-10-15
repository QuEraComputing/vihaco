use external::control as first;
use chain::again::control as second;
struct Cpu;
impl vm::Component for Cpu {}
impl vm::Supports<first::__VihacoDialect> for Cpu {}
impl vm::Supports<second::__VihacoDialect> for Cpu {}
fn main() {}
