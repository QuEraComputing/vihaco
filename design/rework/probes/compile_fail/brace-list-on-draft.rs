use vihaco::{attributes::component, dialect};
dialect! { cpu { Add, } }
#[component]
#[instructions { cpu }]
struct Device;
fn main() {}
