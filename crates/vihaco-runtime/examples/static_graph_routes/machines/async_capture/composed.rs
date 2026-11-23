// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Async capture followed by a join or an acknowledged effect.

use super::{CaptureReady, DigitizerField, Machine, PrepareCapture, ops};
use crate::framework::{
    Await, ChannelDriver, Compute, Describe, FutureDriver, Route, Run, Wait, async_instruction_at,
    await_work, compute, fork_join, step_graph,
};
use crate::machines::simulation::{Scheduled, SimDriver};
use eyre::{Result, eyre};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct CalibratedPeak {
    shot: u32,
    amplitude: i32,
}

struct SplitForCalibration;
impl Compute for SplitForCalibration {
    type Machine = Machine;
    type Input = CaptureReady;
    type Output = ((), CaptureReady);
    const NAME: &'static str = "split calibration from capture analysis";

    fn apply(_: &mut Machine, capture: CaptureReady) -> Result<Self::Output> {
        Ok(((), capture))
    }
}

struct Calibration;
impl Compute for Calibration {
    type Machine = Machine;
    type Input = ();
    type Output = Wait<i16>;
    const NAME: &'static str = "get calibration result handle";

    fn apply(machine: &mut Machine, (): ()) -> Result<Self::Output> {
        machine
            .calibration
            .take()
            .ok_or_else(|| eyre!("missing calibration request"))
    }
}

pub(super) struct Peak {
    shot: u32,
    amplitude: u16,
}

struct MeasurePeak;
impl Compute for MeasurePeak {
    type Machine = Machine;
    type Input = CaptureReady;
    type Output = Peak;
    const NAME: &'static str = "measure capture peak";

    fn apply(machine: &mut Machine, capture: CaptureReady) -> Result<Peak> {
        let amplitude = capture
            .samples
            .into_iter()
            .max()
            .ok_or_else(|| eyre!("empty capture"))?;
        machine.peak_calculations += 1;
        Ok(Peak {
            shot: capture.shot,
            amplitude,
        })
    }
}

struct RecordCalibrated;
impl Compute for RecordCalibrated {
    type Machine = Machine;
    type Input = (i16, Peak);
    type Output = ();
    const NAME: &'static str = "record calibrated peak";

    fn apply(machine: &mut Machine, (offset, peak): Self::Input) -> Result<()> {
        machine.calibrated.push(CalibratedPeak {
            shot: peak.shot,
            amplitude: i32::from(peak.amplitude) - i32::from(offset),
        });
        Ok(())
    }
}

fn calibrated_route() -> Route<
    ops::Capture,
    impl Run<Machine, ops::Capture, ChannelDriver<Machine>, Output = ()> + Describe,
> {
    step_graph::<ops::Capture>()
        .begin(compute::<PrepareCapture>())
        .then_execute_async(async_instruction_at::<DigitizerField>())
        .then(fork_join(
            compute::<SplitForCalibration>(),
            compute::<Calibration>().then(await_work()),
            compute::<MeasurePeak>(),
        ))
        .then(compute::<RecordCalibrated>())
        .finish()
}

struct SubmitArchive<Work>(PhantomData<fn() -> Work>);
impl<Work: 'static> Compute for SubmitArchive<Work> {
    type Machine = Machine<Work>;
    type Input = CaptureReady;
    type Output = Work;
    const NAME: &'static str = "submit capture to archive";

    fn apply(machine: &mut Self::Machine, capture: CaptureReady) -> Result<Self::Output> {
        let acknowledgment = machine
            .archive_ack
            .take()
            .ok_or_else(|| eyre!("missing archive binding"))?;
        machine.archive_outbox.push(capture);
        Ok(acknowledgment)
    }
}

fn archived_route<Work: 'static, D>()
-> Route<ops::Capture, impl Run<Machine<Work>, ops::Capture, D, Output = ()> + Describe>
where
    D: FutureDriver<Machine<Work>> + Await<Machine<Work>, Work, Output = ()>,
{
    step_graph::<ops::Capture>()
        .begin(compute::<PrepareCapture<Work>>())
        .then_execute_async(async_instruction_at::<DigitizerField>())
        .then(compute::<SubmitArchive<Work>>())
        .then(await_work())
        .finish()
}

pub(super) fn print_topologies() {
    calibrated_route()
        .topology()
        .print("Digitizer / Calibrated Capture");
    archived_route::<Wait<()>, ChannelDriver<Machine>>()
        .topology()
        .print("Digitizer / Archived Capture");
}

pub(super) fn calibrated() -> Result<()> {
    use super::{Acquire, Header, Status, fixture};

    let (mut machine, mut runner, device) = fixture();
    let (calibrated, wait) = runner.driver.signal();
    machine.calibration = Some(wait);
    assert_eq!(
        runner.issue_route(&mut machine, calibrated_route())?,
        Status::Parked
    );
    let Acquire {
        header, samples, ..
    } = device.try_recv()?;
    header.complete(Header { sample_count: 3 });
    assert_eq!(runner.wake(&mut machine)?, Status::Parked);
    samples.complete(vec![10, 30, 20]);
    assert_eq!(runner.wake(&mut machine)?, Status::Parked);
    assert_eq!(runner.pc, 4);
    assert_eq!(machine.peak_calculations, 1);
    assert!(machine.calibrated.is_empty());
    assert_eq!(
        machine.digitizer.inspect(|digitizer| digitizer.issued),
        Some(1)
    );

    calibrated.complete(Ok(4));
    assert_eq!(runner.wake(&mut machine)?, Status::Retired { pc: 4 });
    assert_eq!(machine.peak_calculations, 1);
    assert_eq!(
        machine.calibrated,
        [CalibratedPeak {
            shot: 7,
            amplitude: 26
        }]
    );
    assert_eq!(runner.retirements, 1);
    Ok(())
}

pub(super) fn archived(acknowledgment: Result<()>) -> Result<()> {
    use super::{Acquire, Header, Status, fixture};

    let (mut machine, mut runner, device) = fixture();
    let (ack, wait) = runner.driver.signal();
    machine.archive_ack = Some(wait);
    assert_eq!(
        runner.issue_route(&mut machine, archived_route())?,
        Status::Parked
    );
    let Acquire {
        header, samples, ..
    } = device.try_recv()?;
    header.complete(Header { sample_count: 2 });
    assert_eq!(runner.wake(&mut machine)?, Status::Parked);
    samples.complete(vec![10, 20]);
    assert_eq!(runner.wake(&mut machine)?, Status::Parked);
    assert_eq!(runner.pc, 4);
    assert_eq!(runner.retirements, 0);
    assert_eq!(
        machine.digitizer.inspect(|digitizer| digitizer.issued),
        Some(1)
    );
    assert_eq!(
        machine.archive_outbox,
        [CaptureReady {
            shot: 7,
            samples: vec![10, 20]
        }]
    );

    let fault = acknowledgment.as_ref().err().map(ToString::to_string);
    ack.complete(acknowledgment);
    if let Some(fault) = fault {
        assert_eq!(runner.wake(&mut machine).unwrap_err().to_string(), fault);
        assert_eq!(runner.pc, 4);
        assert_eq!(runner.retirements, 0);
    } else {
        assert_eq!(runner.wake(&mut machine)?, Status::Retired { pc: 4 });
        assert_eq!(runner.pc, 5);
        assert_eq!(runner.retirements, 1);
    }
    assert_eq!(machine.archive_outbox.len(), 1);
    Ok(())
}

pub(super) fn simulated_archive() -> Result<()> {
    use super::{Acquire, Header, new_machine};

    let (mut machine, device) = new_machine();
    let driver = SimDriver::new();
    machine.archive_ack = Some(Scheduled {
        tick: 12,
        result: Ok(()),
    });
    let delivered = Rc::new(RefCell::new(None));
    let completion = Rc::clone(&delivered);
    archived_route().start_with(
        &mut machine,
        &driver,
        ops::Capture,
        move |_, driver, result| {
            *completion.borrow_mut() = Some((driver.now(), result));
        },
    );
    let Acquire {
        header, samples, ..
    } = device.try_recv()?;
    header.complete(Header { sample_count: 2 });
    driver.advance_to(&mut machine, 5)?;
    samples.complete(vec![10, 20]);
    driver.advance_to(&mut machine, 11)?;
    assert_eq!(machine.archive_outbox.len(), 1);
    assert!(delivered.borrow().is_none());

    driver.advance_to(&mut machine, 14)?;
    let (tick, result) = delivered.borrow_mut().take().expect("archive completed");
    result?;
    assert_eq!(tick, 12);
    assert_eq!(
        machine.digitizer.inspect(|digitizer| digitizer.issued),
        Some(1)
    );
    driver.advance_to(&mut machine, 20)?;
    assert!(delivered.borrow().is_none());
    Ok(())
}
