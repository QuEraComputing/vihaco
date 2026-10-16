#[path = "../../support/declarations.rs"]
mod declarations;
use declarations::{A, B, Dialect};
use vihaco_runtime::{Component, Execute};
struct Cpu<T>(T);
impl<T> Component for Cpu<T> {}
impl<T, I> Execute<I> for Cpu<T> where T: Copy {
    type Message = ();
    type Effect = ();
    type Error = eyre::Report;
    fn execute(&mut self, _: &I, _: ()) -> eyre::Result<()> { Ok(()) }
}
fn original_bounds<T: Clone>() {
    vihaco_runtime::__private::require_coverage::<Cpu<T>, Dialect>();
}
fn main() { let _: Option<(A, B)> = None; }
