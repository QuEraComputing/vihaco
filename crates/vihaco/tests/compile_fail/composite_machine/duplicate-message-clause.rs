struct Target;
struct Instruction;

vihaco::composite! {
    composite Machine {
        error = Fault;
        target: Target,
    }

    runtime {
        Run(Instruction) => target {
            message none;
            message none;
        }
    }
}

fn main() {}
