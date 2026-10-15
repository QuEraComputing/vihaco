use vihaco::{Execute, attributes::component, dialect};
dialect! { cpu { Add, NewlyAdded, } }
#[component]
#[instructions { cpu::* }]
struct Device;
impl Execute<cpu::Add> for Device {
    type Message = (); type Effect = (); type Error = ();
    fn execute(&mut self, _: &cpu::Add, _: ()) -> Result<(), ()> { Ok(()) }
}
fn main() {}
