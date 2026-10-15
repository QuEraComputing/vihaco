pub mod cpu {
    pub struct Add;
    pub mod syntax { pub struct Add; }
    pub mod __private {
        pub mod __types {
            pub mod runtime { pub mod cpu { pub use crate::cpu::Add; } }
            pub mod surface { pub mod cpu { pub use crate::cpu::syntax::Add; } }
        }
        pub mod __selectors { pub mod cpu { pub use crate::cpu::Add; } }
    }
}
mod authored {
    use crate::cpu as renamed;
    pub mod inventory {
        use super::*;
        pub use renamed::__private as __vihaco_producer_0;
        pub mod __types {
            pub mod runtime { pub use super::super::__vihaco_producer_0::__types::runtime::cpu; }
            pub mod surface { pub use super::super::__vihaco_producer_0::__types::surface::cpu; }
        }
        pub mod __selectors { pub use super::__vihaco_producer_0::__selectors::cpu; }
    }
    pub enum Selected { Add(inventory::__selectors::cpu::Add) }
    pub fn provenance() {
        let _: Option<inventory::__types::runtime::cpu::Add> = None;
        let _: Option<inventory::__types::surface::cpu::Add> = None;
    }
}
