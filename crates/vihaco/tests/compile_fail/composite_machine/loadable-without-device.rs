struct Child;

#[vihaco::composite]
struct BadMachine {
    #[loadable("child")]
    child: Child,
}

fn main() {}
