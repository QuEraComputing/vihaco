// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Run the same Warehouse routes under immediate, timed, and Tokio policies.
//!
//! cargo run -p vihaco-stdlib --features program-image-execution --example logical_routes

#[allow(
    dead_code,
    unused_imports,
    reason = "shared example framework includes other paths"
)]
#[path = "../../vihaco-runtime/examples/static_graph_routes/framework/mod.rs"]
mod framework;
#[path = "logical_routes/machine.rs"]
mod machine;
#[allow(
    dead_code,
    reason = "shared scheduler also supports several machine owners"
)]
#[path = "program_drivers/scheduled/mod.rs"]
mod scheduled;
#[path = "program_drivers/sst.rs"]
mod sst;
#[allow(
    dead_code,
    reason = "shared scheduled driver supports annotated routes as well"
)]
#[path = "../../vihaco-runtime/examples/static_graph_routes/timing.rs"]
mod timing;

use eyre::{Result, eyre};
use framework::{RegisteredTopology, RouteDeclaration};
use machine::{Instruction, Warehouse};
use scheduled::{
    backends::{
        DemoInput, Drive,
        sim::{Sim, SimSubmitter},
        threaded::TokioWorker,
    },
    driver::RouteContext,
    runtime::Runtime,
};
use vihaco_runtime::HasProgram;
use vihaco_stdlib::{Image, Loadable};

fn load() -> Result<Image<Instruction>> {
    sst::load_sections::<Warehouse>(
        "warehouse.sst",
        include_str!("logical_routes/warehouse.sst"),
        &["warehouse"],
    )?
    .pop()
    .ok_or_else(|| eyre!("missing warehouse section"))
}

fn run<D: Drive + DemoInput>(image: Image<Instruction>, mut backend: D) -> Result<WarehouseOutput> {
    let (warehouse, inputs) = Warehouse::fixture();
    for input in inputs {
        backend.schedule_input(input)?;
    }
    let mut runtime = Runtime::new(backend);
    let machine = runtime.execute(Loadable::new(image, warehouse), vec![])?;
    Ok(WarehouseOutput::from(runtime.machine(machine)))
}

#[derive(Debug, PartialEq, Eq)]
struct WarehouseOutput {
    printed: usize,
    labels: Vec<u32>,
    readings: Vec<u16>,
    halt_pc: u32,
}

impl From<&Warehouse> for WarehouseOutput {
    fn from(machine: &Warehouse) -> Self {
        Self {
            printed: machine.printed(),
            labels: machine.labels().to_vec(),
            readings: machine.readings().to_vec(),
            halt_pc: machine.program().pc(),
        }
    }
}

fn print_topology() -> RegisteredTopology {
    machine::print_route::<RouteContext<Warehouse, SimSubmitter>>().registered_topology()
}

fn main() -> Result<()> {
    let print = print_topology();
    print.topology.print("Logical print route");
    let image = load()?;
    let immediate = run(image.clone(), Sim::new())?;
    let timed = run(
        image.clone(),
        Sim::new().delay_edge(&print, "label_to_records", 12)?,
    )?;
    let tokio = run(image, TokioWorker::new()?)?;
    assert_eq!(immediate, timed);
    assert_eq!(timed, tokio);
    println!("Immediate simulation, timed simulation, and Tokio produced {tokio:?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vihaco_stdlib::RunState;

    #[test]
    fn edge_policy_rejects_a_node_name_before_issue() {
        assert!(
            Sim::new()
                .delay_edge(&print_topology(), "print", 12)
                .is_err()
        );
    }

    #[test]
    fn backend_decides_whether_the_same_print_route_parks() -> Result<()> {
        for delay in [0, 12] {
            let (machine, _) = Warehouse::fixture();
            let mut runtime = Runtime::new(Sim::starting_at(5).delay_edge(
                &print_topology(),
                "label_to_records",
                delay,
            )?);
            let id = runtime.load(Loadable::new(load()?, machine));
            runtime.schedule_entry(id, vec![])?;
            runtime.advance_one()?; // Enter.
            runtime.advance_one()?; // Issue the print once.
            assert_eq!(runtime.machine(id).printed(), 1);
            if delay == 0 {
                assert_eq!(runtime.machine(id).labels(), &[42]);
                assert_eq!(runtime.machine(id).program().pc(), 1);
            } else {
                assert!(runtime.machine(id).labels().is_empty());
                assert_eq!(runtime.machine(id).program().pc(), 0);
                assert!(matches!(
                    runtime.machine(id).program().run_state(),
                    RunState::Other(_)
                ));
                runtime.advance_one()?; // Deliver the owned label.
                assert_eq!(runtime.backend().now(), 17);
                assert_eq!(runtime.machine(id).printed(), 1);
                assert_eq!(runtime.machine(id).labels(), &[42]);
                assert_eq!(runtime.machine(id).program().pc(), 1);
            }
        }
        Ok(())
    }

    #[test]
    fn all_hooks_run_on_either_backend() -> Result<()> {
        let image = load()?;
        let expected = WarehouseOutput {
            printed: 1,
            labels: vec![42],
            readings: vec![40, 41],
            halt_pc: 3,
        };
        assert_eq!(run(image.clone(), Sim::new())?, expected);
        assert_eq!(
            run(
                image.clone(),
                Sim::new().delay_edge(&print_topology(), "label_to_records", 12)?
            )?,
            expected
        );
        assert_eq!(run(image, TokioWorker::new()?)?, expected);
        Ok(())
    }
}
