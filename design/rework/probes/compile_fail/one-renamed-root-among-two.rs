use vihaco::{Execute, attributes::component, dialect};
mod vendor_a {
    use super::*;
    dialect! { #[vihaco(inventory_id = alias_a)] cpu { Add, } }
}
mod vendor_b {
    use super::*;
    dialect! { #[vihaco(inventory_id = alias_b)] cpu { Halt, } }
}
use vendor_a::cpu as a;
use vendor_b::cpu as b;
#[component]
#[instructions { a::*, vendor_b::cpu::* }]
struct Device;
impl Execute<a::Add> for Device {
    type Message = (); type Effect = (); type Error = ();
    fn execute(&mut self, _: &a::Add, _: ()) -> Result<(), ()> { Ok(()) }
}
impl Execute<b::Halt> for Device {
    type Message = (); type Effect = (); type Error = ();
    fn execute(&mut self, _: &b::Halt, _: ()) -> Result<(), ()> { Ok(()) }
}
fn main() {
    let _: Option<__vihaco_inventory_device::__selectors::cpu::Halt> = None;
}
