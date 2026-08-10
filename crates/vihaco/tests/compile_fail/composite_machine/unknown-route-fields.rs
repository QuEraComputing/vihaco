struct Target;
struct Instruction;

vihaco::composite! {
    composite Machine {
        error = Fault;
        target: Target,
    }

    runtime {
        Run(Instruction) => missing_target {
            message from missing_message;
            effects {
                observe missing_observer;
                absorb with missing_sink;
            }
        }
    }
}

fn main() {}
