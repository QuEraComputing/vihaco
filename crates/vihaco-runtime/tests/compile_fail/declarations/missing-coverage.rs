#[path = "../../support/declarations.rs"]
mod declarations;
use declarations::{A, Dialect};
use vihaco_runtime::{Component, Execute};
struct Cpu;
impl Component for Cpu {}
impl Execute<A> for Cpu {
    type Message = ();
    type Effect = ();
    type Error = eyre::Report;
    fn execute(&mut self, _: &A, _: ()) -> eyre::Result<()> { Ok(()) }
}
fn original_bounds() {
    vihaco_runtime::__private::require_coverage::<Cpu, Dialect>();
}
fn main() {}
