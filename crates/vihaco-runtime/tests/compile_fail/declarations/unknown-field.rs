#[path = "../../support/declarations.rs"]
mod declarations;
use declarations::Cpu;
use vihaco_runtime::__private::Field;
type Wrong = <Cpu<'static> as Field<0>>::Child;
fn main() { let _: Option<Wrong> = None; }
