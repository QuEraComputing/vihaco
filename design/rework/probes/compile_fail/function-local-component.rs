use vihaco::{Execute, attributes::component, dialect};
dialect! { cpu { Add, } }
fn main() {
    #[component]
    #[instructions { cpu::* }]
    struct Local;
    impl Execute<cpu::Add> for Local {
        type Message = (); type Effect = (); type Error = ();
        fn execute(&mut self, _: &cpu::Add, _: ()) -> Result<(), ()> { Ok(()) }
    }
}
