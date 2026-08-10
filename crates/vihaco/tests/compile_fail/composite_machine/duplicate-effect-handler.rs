struct Target;
struct Instruction;

vihaco::composite! {
    composite Machine {
        error = Fault;
        target: Target,
        sink: Sink,
    }

    runtime {
        Run(Instruction) => target {
            message none;
            effects {
                absorb with sink;
                handle with handle_effect;
            }
        }
    }
}

fn main() {}
