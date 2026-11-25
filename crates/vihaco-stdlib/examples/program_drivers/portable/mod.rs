// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Load two sections and run the same machine on a virtual clock and Tokio.

mod backends;
mod digitizer;
mod owner;
mod runtime;
mod sst;
mod tasks;

use backends::{
    DemoInput, Drive, Input,
    sim::{Sim, SimSubmitter},
    threaded::{TokioSubmitter, run_on_workers},
};
use digitizer::{Instruction, fixture, topology};
use eyre::{Result, ensure, eyre};
use runtime::Runtime;
use std::thread::{self, ThreadId};
use vihaco_runtime::HasProgram;
use vihaco_stdlib::{Image, Loadable};

struct Program {
    section: &'static str,
    image: Image<Instruction>,
    samples: Vec<(u64, u16)>,
}

fn programs() -> Result<Vec<Program>> {
    Ok(sst::SECTIONS
        .into_iter()
        .zip(sst::load(sst::SOURCE)?)
        .zip([(20, 23), (4, 29)])
        .map(|((section, image), (input_delay, sample))| Program {
            section,
            image,
            samples: vec![(input_delay, sample)],
        })
        .collect())
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    section: &'static str,
    sample: u16,
    captures: usize,
    halt_pc: u32,
}

#[derive(Debug)]
struct Report {
    worker: ThreadId,
    outcomes: Vec<Outcome>,
}

/// Load and enqueue the sections assigned to this worker, then start the runtime.
fn run_sections<D: Drive + DemoInput>(
    mut backend: D,
    programs: Vec<Program>,
) -> Result<(Report, Runtime<D>)> {
    let mut names = Vec::new();
    let mut loadables = Vec::new();
    for Program {
        section,
        image,
        samples,
    } in programs
    {
        let (machine, ports) = fixture(samples.len());
        names.push(section);
        loadables.push(Loadable::new(image, machine));
        for ((delay, sample), port) in samples.into_iter().zip(ports) {
            backend.schedule_input(Input {
                delay,
                sample,
                port,
            });
        }
    }

    let mut runtime = Runtime::new(backend);
    for loadable in loadables {
        let machine = runtime.load(loadable);
        runtime.schedule_entry(machine, Vec::new())?;
    }
    let halted = runtime.run()?;

    // Check the demo results after the runtime finishes.
    let outcomes = halted
        .into_iter()
        .map(|index| {
            let machine = runtime.machine(index);
            let (captures, sample) = machine
                .digitizer_state()
                .and_then(|(captures, sample)| sample.map(|sample| (captures, sample)))
                .ok_or_else(|| eyre!("program halted without storing a sample"))?;
            Ok(Outcome {
                section: names[index],
                sample,
                captures,
                halt_pc: machine.program().pc(),
            })
        })
        .collect::<Result<_>>()?;
    Ok((
        Report {
            worker: thread::current().id(),
            outcomes,
        },
        runtime,
    ))
}

fn emulate() -> Result<(Report, u64)> {
    let (report, runtime) = run_sections(Sim::new(), programs()?)?;
    Ok((report, runtime.backend().now()))
}

fn run_tokio(workers: usize) -> Result<Vec<Report>> {
    run_on_workers(workers, programs()?, |backend, programs| {
        let (report, _) = run_sections(backend, programs)?;
        Ok(report)
    })
}

fn sorted_outcomes(reports: Vec<Report>) -> Vec<Outcome> {
    let mut outcomes = reports
        .into_iter()
        .flat_map(|report| report.outcomes)
        .collect::<Vec<_>>();
    outcomes.sort_by_key(|outcome| outcome.section);
    outcomes
}

fn print(report: &Report) {
    for outcome in &report.outcomes {
        println!(
            "{:?} {}: sample {}, {} capture, halted at PC {}",
            report.worker, outcome.section, outcome.sample, outcome.captures, outcome.halt_pc
        );
    }
}

pub(super) fn demo(workers: usize) -> Result<()> {
    let graph = topology::<SimSubmitter>();
    ensure!(graph == topology::<TokioSubmitter>(), "topologies differ");
    println!("Capture topology: {graph:?}");

    let (sim, tick) = emulate()?;
    println!("Virtual time finished at tick {tick}");
    print(&sim);
    let threaded = run_tokio(workers)?;
    println!("Tokio with {workers} OS workers");
    for report in &threaded {
        print(report);
    }
    ensure!(
        sorted_outcomes(vec![sim]) == sorted_outcomes(threaded),
        "machine outcomes differ"
    );
    println!("Same topology and machine outcomes");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_topology_and_program_outcomes_on_sim_and_tokio_workers() -> Result<()> {
        assert_eq!(topology::<SimSubmitter>(), topology::<TokioSubmitter>());
        let (sim, tick) = emulate()?;
        assert_eq!(tick, 20);
        assert_eq!(
            sim.outcomes
                .iter()
                .map(|outcome| outcome.section)
                .collect::<Vec<_>>(),
            ["digitizer-b", "digitizer-a"]
        );
        let expected = vec![
            Outcome {
                section: "digitizer-a",
                sample: 23,
                captures: 1,
                halt_pc: 1,
            },
            Outcome {
                section: "digitizer-b",
                sample: 29,
                captures: 1,
                halt_pc: 3,
            },
        ];
        assert_eq!(sorted_outcomes(vec![sim]), expected);

        let threaded = run_tokio(2)?;
        assert_eq!(threaded.len(), 2);
        assert_ne!(threaded[0].worker, threaded[1].worker);
        assert_eq!(threaded[0].outcomes.len(), 1);
        assert_eq!(threaded[1].outcomes.len(), 1);
        assert_eq!(sorted_outcomes(threaded), expected);
        Ok(())
    }

    #[test]
    fn entry_is_queued_and_one_program_can_park_twice() -> Result<()> {
        let source = sst::SOURCE.replacen(
            "    digitizer::acquisition::capture",
            "    digitizer::acquisition::capture\n    digitizer::acquisition::capture",
            1,
        );
        let [image, _] = sst::load(&source)?;
        let mut sim = Sim::new();
        let (machine, ports) = fixture(2);
        for ((delay, sample), port) in [(4, 19), (8, 23)].into_iter().zip(ports) {
            sim.schedule_input(Input {
                delay,
                sample,
                port,
            });
        }

        let mut runtime = Runtime::new(sim);
        let id = runtime.load(Loadable::new(image, machine));
        runtime.schedule_entry(id, Vec::new())?;
        assert!(runtime.machine(id).program().is_halted());
        assert_eq!(runtime.machine(id).digitizer_state(), Some((0, None)));
        assert_eq!(runtime.run()?, [id]);
        assert_eq!(runtime.machine(id).digitizer_state(), Some((2, Some(23))));
        assert_eq!(runtime.machine(id).program().pc(), 2);
        assert_eq!(runtime.backend().now(), 8);
        Ok(())
    }

    #[test]
    fn one_tokio_worker_services_both_machines() -> Result<()> {
        let (sim, _) = emulate()?;
        let threaded = run_tokio(1)?;
        assert_eq!(threaded.len(), 1);
        assert_eq!(threaded[0].outcomes.len(), 2);
        assert_eq!(sorted_outcomes(threaded), sorted_outcomes(vec![sim]));
        Ok(())
    }
}
