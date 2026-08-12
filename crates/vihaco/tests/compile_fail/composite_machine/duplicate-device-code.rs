struct DemoDevice;

vihaco::composite! {
composite BadMachine {
    #[device(0x01)]
    a: DemoDevice,
    #[device(0x01)]
    b: DemoDevice,
}
}

fn main() {}
