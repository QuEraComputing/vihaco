// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{
    framework::ChannelDriver, machine::driver, simulation::SimDriver, tokio_threads::TokioThreads,
};
use eyre::Result;
use std::sync::Arc;
use tokio::sync::Barrier;
use vihaco_stdlib::program_driver::{ProgramDriver, ProgramStop, RunCommand};

fn pending(stop: ProgramStop<u64>, expected_pc: u32) -> u64 {
    let ProgramStop::Pending { pc, work } = stop else {
        panic!("expected pending work, got {stop:?}");
    };
    assert_eq!(pc, expected_pc);
    work
}

pub(super) fn debugger() -> Result<()> {
    let mut vm = driver(SimDriver::new(), 23);
    vm.set_breakpoint(2);
    vm.set_breakpoint(3);
    vm.start((), &[])?;
    assert_eq!(
        vm.run((), RunCommand::Continue)?,
        ProgramStop::Breakpoint { pc: 2 }
    );

    let work = pending(vm.run((), RunCommand::SingleStep)?, 2);
    assert_eq!(vm.retirements(), 1);
    // Repeating a host run command leaves the in-flight step's authorization intact.
    assert_eq!(pending(vm.run((), RunCommand::Continue)?, 2), work);
    let reply = pending(vm.deliver((), work, ())?, 2);
    assert_ne!(reply, work);
    assert!(vm.deliver((), work, ()).is_err());
    assert_eq!(vm.retirements(), 1);
    assert_eq!(vm.deliver((), reply, ())?, ProgramStop::Retired { pc: 2 });
    assert_eq!(vm.retirements(), 2);
    assert_eq!(
        vm.run((), RunCommand::Continue)?,
        ProgramStop::Breakpoint { pc: 3 }
    );
    assert_eq!(
        vm.run((), RunCommand::Continue)?,
        ProgramStop::Halted { pc: 3 }
    );
    println!("Debugger: a parked single-step retired before the next breakpoint");
    Ok(())
}

pub(super) fn channel() -> Result<()> {
    let mut vm = driver(ChannelDriver::new(), 0);
    vm.start((), &[])?;
    let work = pending(vm.run((), RunCommand::Continue)?, 2);
    assert_eq!(vm.machine().execution.pc(), 2);
    assert!(vm.machine().execution.stack().is_empty());
    assert!(vm.deliver((), work + 1, Ok(99)).is_err());
    assert_eq!(vm.deliver((), work, Ok(19))?, ProgramStop::Halted { pc: 3 });
    assert_eq!(vm.machine().sensor.issued, 1);
    assert_eq!(vm.machine().execution.stack(), &[19]);
    assert_eq!(vm.retirements(), 3);
    println!("Channel: an external sample resumed the route without reissuing the sensor");
    Ok(())
}

pub(super) fn tokio_threads() -> Result<()> {
    let gate = Arc::new(Barrier::new(2));
    let mut fleet = TokioThreads::new(&[41, 42], Some(gate))?;
    fleet.start(0, &[])?;
    fleet.start(1, &[])?;
    let work0 = pending(fleet.run(0, RunCommand::Continue)?, 2);
    let work1 = pending(fleet.run(1, RunCommand::Continue)?, 2);
    // Both local futures must reach the barrier before either machine can finish.
    assert_eq!(
        fleet.deliver_together(&[(0, work0), (1, work1)])?,
        [ProgramStop::Halted { pc: 3 }; 2]
    );
    let (thread0, stack0) = fleet.inspect(0)?;
    let (thread1, stack1) = fleet.inspect(1)?;
    assert_ne!(thread0, thread1);
    assert_eq!(stack0, [41]);
    assert_eq!(stack1, [42]);
    println!("Tokio: two composites completed scheduled work on distinct owner threads");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eyre::eyre;

    #[test]
    fn all_five_program_drivers_run() -> Result<()> {
        super::super::main()
    }

    #[test]
    fn tokio_driver_accepts_an_individual_delivery() -> Result<()> {
        let mut fleet = TokioThreads::new(&[41, 42], None)?;
        fleet.start(1, &[])?;
        let work = pending(fleet.run(1, RunCommand::Continue)?, 2);
        assert_eq!(fleet.deliver(1, work, ())?, ProgramStop::Halted { pc: 3 });
        assert_eq!(fleet.inspect(1)?.1, [42]);
        Ok(())
    }

    #[test]
    fn deferred_fault_reports_at_the_origin_without_retiring() -> Result<()> {
        let mut vm = driver(ChannelDriver::new(), 0);
        vm.start((), &[])?;
        let work = pending(vm.run((), RunCommand::Continue)?, 2);
        let error = vm
            .deliver((), work, Err(eyre!("sensor failed")))
            .unwrap_err();
        assert_eq!(error.to_string(), "sensor failed");
        assert_eq!(vm.machine().execution.pc(), 2);
        assert_eq!(vm.machine().sensor.issued, 1);
        assert_eq!(vm.retirements(), 1);
        assert!(vm.run((), RunCommand::Continue).is_err());
        Ok(())
    }
}
