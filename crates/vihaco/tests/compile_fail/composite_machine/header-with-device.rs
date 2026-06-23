struct HeaderDevice;

#[vihaco::composite]
struct BadMachine {
    #[header]
    #[device(0x01)]
    header: HeaderDevice,
}

fn main() {}
