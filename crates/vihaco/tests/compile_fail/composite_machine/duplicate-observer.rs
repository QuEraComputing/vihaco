struct Target;
struct Instruction;

vihaco::composite! {
    composite Machine {
        error = Fault;
        target: Target,
        observer: Observer,
        sink: Sink,
    }

    runtime {
        Run(Instruction) => target {
            message none;
            effects {
                observe observer, observer;
                absorb with sink;
            }
        }
    }
}

fn main() {}
