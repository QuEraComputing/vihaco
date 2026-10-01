#[path = "../../support/declarations.rs"]
mod declarations;
use declarations::Dialect;
use vihaco_runtime::__private::{Capabilities, Description, DialectSpec, validate};
struct Bad<T>(T);
impl<T> Capabilities for Bad<T> {
    const DESCRIPTION: &'static Description = &Description::Component(&[Dialect::LEAVES, Dialect::LEAVES]);
}
const _: () = validate(Bad::<u8>::DESCRIPTION);
fn main() {}
