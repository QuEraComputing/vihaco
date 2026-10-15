use vihaco::{Execute, attributes::component};
macro_rules! execute {
    ($device:ty, $leaf:ty) => {
        impl Execute<$leaf> for $device {
            type Message = (); type Effect = (); type Error = ();
            fn execute(&mut self, _: &$leaf, _: ()) -> Result<(), ()> { Ok(()) }
        }
    };
}
#[component]
#[instructions { vihaco_inventory_producer_fixture::nested::arith::* }]
struct BareExternal;
execute!(BareExternal, vihaco_inventory_producer_fixture::nested::arith::Add);
execute!(BareExternal, vihaco_inventory_producer_fixture::nested::arith::Halt);

fn main() {}
