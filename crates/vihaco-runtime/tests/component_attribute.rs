// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "component-attribute")]

use vihaco_runtime::{Execute, attributes::component, dialect};

mod renamed_root {
    pub use vihaco_runtime::*;
}

dialect! {
    direct {
        Run,
    }
}

#[component]
#[instructions { direct::Run }]
#[vihaco(crate = crate::renamed_root)]
struct Device;

impl Execute<direct::Run> for Device {
    type Message = ();
    type Effect = ();
    type Error = ();

    fn execute(&mut self, _: &direct::Run, _: ()) -> Result<(), ()> {
        Ok(())
    }
}

#[test]
fn direct_runtime_use_and_crate_override() {
    let mut device = Device;
    assert_eq!(
        Execute::<direct::Run>::execute(&mut device, &direct::Run, ()),
        Ok(())
    );
}
