#[path = "../../support/declarations.rs"]
mod declarations;
use declarations::{Cpu, Dialect};
impl vihaco_runtime::__private::Supports<Dialect> for Cpu<'_> {}
fn main() {}
