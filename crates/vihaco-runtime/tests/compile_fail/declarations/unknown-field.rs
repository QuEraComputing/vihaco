#[path = "../../support/declarations.rs"]
mod declarations;
use declarations::Cpu;
use vihaco_runtime::Field;
struct Unknown;
type Wrong = <Cpu<'static> as Field<Unknown>>::Child;
fn main() { let _: Option<Wrong> = None; }
