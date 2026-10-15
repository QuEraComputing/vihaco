use vihaco::{Execute, attributes::component};
mod fake {
    pub struct Add;
    pub mod __private {
        pub mod __types {
            pub mod runtime { pub mod fake { pub use crate::fake::Add; } }
            pub mod surface { pub mod fake { pub use crate::fake::Add; } }
        }
        #[macro_export]
        macro_rules! __duplicate_entries {
            (callback: $callback:ident, state: { $($state:tt)* },) => {
                $callback! {
                    state: { $($state)* },
                    inventory: {
                        entry { selector: { fake::Add }, type_route: { fake::Add }, source_steps: {}, },
                        entry { selector: { fake::Add }, type_route: { fake::Add }, source_steps: {}, },
                    },
                }
            };
        }
        pub use __duplicate_entries as __entries;
    }
}
#[component]
#[instructions { fake::* }]
struct Device;
impl Execute<fake::Add> for Device {
    type Message = (); type Effect = (); type Error = ();
    fn execute(&mut self, _: &fake::Add, _: ()) -> Result<(), ()> { Ok(()) }
}
fn main() {}
