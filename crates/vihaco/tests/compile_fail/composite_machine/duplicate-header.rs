#[derive(Default)]
struct Header;

#[vihaco::composite]
struct BadMachine {
    #[header]
    first: Header,

    #[header]
    second: Header,
}

fn main() {}
