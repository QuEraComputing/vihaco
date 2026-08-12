struct Child;

vihaco::composite! {
composite BadMachine {
    #[loadable("child")]
    child: Child,
}
}

fn main() {}
