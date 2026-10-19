use vihaco_runtime::composite;

#[composite]
struct MissingDevice {
    #[program]
    state: u8,
}

#[composite]
struct MultiplePrograms {
    #[device(1)]
    #[program]
    a: u8,
    #[device(2)]
    #[program]
    b: u8,
}

fn main() {}
