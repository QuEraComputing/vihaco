// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! One async capture carried through a graph invocation and retired by its caller.

mod composed;

use crate::framework::{
    AsyncExecute, AsyncSlot, ChannelDriver, Compute, Describe, Receive, Route, Run, Wait,
    async_instruction_at, compute, receive_effect_at, step_graph,
};
use eyre::{Result, ensure, eyre};
use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    task::{Poll, Waker},
};
use vihaco_runtime::{Field, dialect};

#[dialect]
#[vihaco(crate = ::vihaco_runtime)]
pub(crate) mod ops {
    #[instruction]
    pub struct Capture;
}

pub(crate) struct CaptureMessage {
    shot: u32,
}

struct Header {
    sample_count: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CaptureReady {
    shot: u32,
    samples: Vec<u16>,
}

pub(crate) struct Digitizer {
    commands: Sender<Acquire>,
    issued: usize,
    completed: Vec<CaptureReady>,
}

impl AsyncExecute<ops::Capture> for Digitizer {
    type Message = CaptureMessage;
    type Effect = CaptureReady;
    type Error = eyre::Report;

    async fn execute(&mut self, _: &ops::Capture, message: CaptureMessage) -> Result<CaptureReady> {
        let (send_header, header) = response();
        let (send_samples, samples) = response();

        self.commands
            .send(Acquire {
                shot: message.shot,
                header: send_header,
                samples: send_samples,
            })
            .map_err(|_| eyre!("digitizer disconnected"))?;

        self.issued += 1;
        let header = header.await;
        let samples = samples.await;
        ensure!(
            samples.len() == header.sample_count,
            "sample count mismatch"
        );
        Ok(CaptureReady {
            shot: message.shot,
            samples,
        })
    }
}

impl Receive<CaptureReady> for Digitizer {
    fn receive(&mut self, effect: CaptureReady) -> Result<()> {
        self.completed.push(effect);
        Ok(())
    }
}

pub(crate) struct Machine<ArchiveWork = Wait<()>> {
    digitizer: AsyncSlot<Digitizer>,
    next_shot: Option<u32>,
    calibration: Option<Wait<i16>>,
    peak_calculations: usize,
    calibrated: Vec<composed::CalibratedPeak>,
    archive_ack: Option<ArchiveWork>,
    archive_outbox: Vec<CaptureReady>,
}

// The current composite macro requires synchronous Execute coverage. The
// prototype declares the same field route directly to exercise async-only coverage.
struct DigitizerField;
impl<ArchiveWork> Field<DigitizerField> for Machine<ArchiveWork> {
    type Child = AsyncSlot<Digitizer>;
    const SOURCE: &'static [&'static str] = &["digitizer"];

    fn get(&self) -> &Self::Child {
        &self.digitizer
    }
    fn get_mut(&mut self) -> &mut Self::Child {
        &mut self.digitizer
    }
}

struct PrepareCapture<ArchiveWork = Wait<()>>(PhantomData<fn() -> ArchiveWork>);
impl<ArchiveWork> Compute for PrepareCapture<ArchiveWork> {
    type Machine = Machine<ArchiveWork>;
    type Input = ops::Capture;
    type Output = (ops::Capture, CaptureMessage);
    const NAME: &'static str = "prepare capture message";

    fn apply(machine: &mut Self::Machine, instruction: ops::Capture) -> Result<Self::Output> {
        let shot = machine
            .next_shot
            .take()
            .ok_or_else(|| eyre!("missing shot operand"))?;
        Ok((instruction, CaptureMessage { shot }))
    }
}

fn capture_route() -> Route<
    ops::Capture,
    impl Run<Machine, ops::Capture, ChannelDriver<Machine>, Output = ()> + Describe,
> {
    step_graph::<ops::Capture>()
        .begin(compute::<PrepareCapture>())
        .then_execute_async(async_instruction_at::<DigitizerField>())
        .then(receive_effect_at::<DigitizerField>())
        .finish()
}

// A single sequential instruction slot shows where issue and retirement
// would connect to the program runner.
struct Checked {
    pc: usize,
    in_flight: bool,
    retirements: usize,
    driver: ChannelDriver<Machine>,
    completion: Rc<RefCell<Option<Result<()>>>>,
}

#[derive(Debug, PartialEq, Eq)]
enum Status {
    Parked,
    Retired { pc: usize },
}

impl Checked {
    fn issue(&mut self, machine: &mut Machine) -> Result<Status> {
        self.issue_route(machine, capture_route())
    }

    fn issue_route(
        &mut self,
        machine: &mut Machine,
        route: Route<
            ops::Capture,
            impl Run<Machine, ops::Capture, ChannelDriver<Machine>, Output = ()>,
        >,
    ) -> Result<Status> {
        ensure!(!self.in_flight, "route already in flight");
        self.in_flight = true;
        let completion = Rc::clone(&self.completion);
        route.start_with(machine, &self.driver, ops::Capture, move |_, _, result| {
            *completion.borrow_mut() = Some(result);
        });
        self.observe()
    }

    fn wake(&mut self, machine: &mut Machine) -> Result<Status> {
        ensure!(self.in_flight, "no route in flight");
        self.driver.poll_ready(machine);
        self.observe()
    }

    fn observe(&mut self) -> Result<Status> {
        let Some(result) = self.completion.borrow_mut().take() else {
            return Ok(Status::Parked);
        };
        self.in_flight = false;
        result?;
        let pc = self.pc;
        self.pc += 1;
        self.retirements += 1;
        Ok(Status::Retired { pc })
    }
}

struct Acquire {
    shot: u32,
    header: Signal<Header>,
    samples: Signal<Vec<u16>>,
}

struct Response<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

struct Signal<T>(Arc<Mutex<Response<T>>>);

impl<T> Signal<T> {
    fn complete(self, value: T) {
        let waker = {
            let mut response = self.0.lock().unwrap();
            response.value = Some(value);
            response.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

fn response<T>() -> (Signal<T>, impl Future<Output = T>) {
    let shared = Arc::new(Mutex::new(Response {
        value: None,
        waker: None,
    }));
    let signal = Signal(Arc::clone(&shared));
    let future = poll_fn(move |context| {
        let mut response = shared.lock().unwrap();
        if let Some(value) = response.value.take() {
            Poll::Ready(value)
        } else {
            response.waker = Some(context.waker().clone());
            Poll::Pending
        }
    });
    (signal, future)
}

fn new_machine<ArchiveWork>() -> (Machine<ArchiveWork>, Receiver<Acquire>) {
    let (commands, device) = mpsc::channel();
    let machine = Machine {
        digitizer: AsyncSlot::new(Digitizer {
            commands,
            issued: 0,
            completed: Vec::new(),
        }),
        next_shot: Some(7),
        calibration: None,
        peak_calculations: 0,
        calibrated: Vec::new(),
        archive_ack: None,
        archive_outbox: Vec::new(),
    };
    (machine, device)
}

fn fixture() -> (Machine, Checked, Receiver<Acquire>) {
    let (machine, device) = new_machine();
    let runner = Checked {
        pc: 4,
        in_flight: false,
        retirements: 0,
        driver: ChannelDriver::new(),
        completion: Rc::default(),
    };
    (machine, runner, device)
}

pub(crate) fn demo() -> Result<()> {
    capture_route()
        .topology()
        .print("Digitizer / Async Capture");
    composed::print_topologies();
    capture()?;
    composed::calibrated()?;
    composed::archived(Ok(()))?;
    composed::simulated_archive()
}

fn capture() -> Result<()> {
    let route = capture_route();
    assert_eq!(std::mem::size_of_val(&route), 0);
    assert_eq!(route.topology().nodes.len(), 3);

    let (mut machine, mut runner, device) = fixture();
    assert_eq!(runner.issue(&mut machine)?, Status::Parked);
    assert_eq!(runner.pc, 4);
    assert_eq!(machine.next_shot, None);
    assert!(runner.issue(&mut machine).is_err());
    let Acquire {
        shot,
        header,
        samples,
    } = device.try_recv()?;
    assert_eq!(shot, 7);

    header.complete(Header { sample_count: 3 });
    assert_eq!(runner.wake(&mut machine)?, Status::Parked);
    assert_eq!(runner.pc, 4);
    assert_eq!(
        machine.digitizer.inspect(|digitizer| digitizer.issued),
        None
    );
    assert!(device.try_recv().is_err());

    samples.complete(vec![10, 20, 30]);
    assert_eq!(runner.wake(&mut machine)?, Status::Retired { pc: 4 });
    assert_eq!(runner.pc, 5);
    assert_eq!(runner.retirements, 1);
    machine
        .digitizer
        .inspect(|digitizer| {
            assert_eq!(digitizer.issued, 1);
            assert_eq!(
                digitizer.completed,
                [CaptureReady {
                    shot: 7,
                    samples: vec![10, 20, 30]
                }]
            );
        })
        .expect("component restored before effect delivery");
    assert!(runner.wake(&mut machine).is_err());
    Ok(())
}

#[cfg(test)]
fn deferred_error() -> Result<()> {
    let (mut machine, mut runner, device) = fixture();
    assert_eq!(runner.issue(&mut machine)?, Status::Parked);
    let Acquire {
        header, samples, ..
    } = device.try_recv()?;
    header.complete(Header { sample_count: 3 });
    assert_eq!(runner.wake(&mut machine)?, Status::Parked);
    samples.complete(vec![10, 20]);
    assert_eq!(
        runner.wake(&mut machine).unwrap_err().to_string(),
        "sample count mismatch"
    );
    assert_eq!(runner.pc, 4);
    assert_eq!(runner.retirements, 0);
    assert_eq!(
        machine
            .digitizer
            .inspect(|digitizer| digitizer.completed.len()),
        Some(0)
    );
    assert!(!runner.in_flight);
    Ok(())
}

#[cfg(test)]
fn drop_parked_route() -> Result<()> {
    let (mut machine, mut runner, device) = fixture();
    assert_eq!(runner.issue(&mut machine)?, Status::Parked);
    let _outstanding = device.try_recv()?;
    assert_eq!(
        machine.digitizer.inspect(|digitizer| digitizer.issued),
        None
    );
    drop(runner);
    assert_eq!(
        machine.digitizer.inspect(|digitizer| digitizer.issued),
        Some(1)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn async_route_resumes_and_retires_once() -> Result<()> {
        capture()
    }

    #[test]
    fn async_route_fault_stays_at_originating_pc() -> Result<()> {
        deferred_error()
    }

    #[test]
    fn dropping_a_parked_route_returns_its_component() -> Result<()> {
        drop_parked_route()
    }

    #[test]
    fn async_capture_joins_calibration() -> Result<()> {
        composed::calibrated()
    }

    #[test]
    fn async_capture_waits_for_archive_acknowledgment() -> Result<()> {
        composed::archived(Ok(()))
    }

    #[test]
    fn async_capture_waits_for_simulated_archive() -> Result<()> {
        composed::simulated_archive()
    }

    #[test]
    fn archive_failure_does_not_retire_capture() -> Result<()> {
        composed::archived(Err(eyre!("archive rejected the write")))
    }
}
