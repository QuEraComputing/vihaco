#[path = "../../support/declarations.rs"]
mod declarations;
use declarations::{Cpu, Extra};
fn check() { vihaco_runtime::__private::require_available::<Cpu, Extra>(); }
fn main() {}
