// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{
    channel::{EndpointId, ReceiveCompletion, ReceiveContinuation, SharedTransport, Transport},
    clock::{ClockedComponent, GlobalClock, GlobalTick, GlobalTicksPerLocalCycle, Schedule},
    cpu::{Cpu, CpuEvent, CpuFault, RuntimeInstruction},
};
use std::collections::HashMap;

// ===========================================================================================
// === AUTHOR: the top-level composite and its root event loop ===============================
// ===========================================================================================
//
// `HeterogeneousMachine` is the single top-level composite and the runtime root. It has no local
// executable instruction section and does not implement the child instruction-dispatch role; it
// owns the root event loop, interprets the machine-specific `MachineEvent` sum, and maps each
// child-local scheduling request into the appropriate root event variant. The reusable `Cpu` never
// constructs a `MachineEvent` or names itself `CpuA`/`CpuB`; parent routing attaches that identity.

/// Which CPU an event or waiter refers to. The reusable CPU is oblivious to this tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpuId {
    A,
    B,
}

/// The machine-specific owned event sum interpreted by the root. The generic `GlobalClock` is
/// parameterized over this type and never inspects it.
#[derive(Debug, Clone, Copy)]
pub enum MachineEvent {
    /// Run the next instruction of the identified CPU.
    Step(CpuId),
    /// Resume a receive at the receiver's next local clock boundary.
    Resume {
        id: CpuId,
        continuation: ReceiveContinuation,
        value: i64,
    },
}

/// How the machine terminated.
#[derive(Debug, PartialEq, Eq)]
pub enum RunOutcome {
    /// Both programs finished with no lost value, stale continuation, or pending event.
    Completed,
    /// Every runnable CPU is parked and no delivery can satisfy any continuation.
    Deadlock,
}

pub struct HeterogeneousMachine {
    pub clock: GlobalClock<MachineEvent>,
    pub transport: SharedTransport<i64>,
    pub ticks_per_local_cycle: HashMap<CpuId, GlobalTicksPerLocalCycle>,
    pub cpu_a: Cpu,
    pub cpu_b: Cpu,
    /// A human-readable record of the deterministic global trace, asserted by the driver.
    pub execution_trace: Vec<String>,
}

impl HeterogeneousMachine {
    fn cpu_mut(&mut self, id: CpuId) -> &mut Cpu {
        match id {
            CpuId::A => &mut self.cpu_a,
            CpuId::B => &mut self.cpu_b,
        }
    }

    fn cpu_ref(&self, id: CpuId) -> &Cpu {
        match id {
            CpuId::A => &self.cpu_a,
            CpuId::B => &self.cpu_b,
        }
    }

    fn ticks_per_local_cycle(&self, id: CpuId) -> Result<GlobalTicksPerLocalCycle, CpuFault> {
        self.ticks_per_local_cycle
            .get(&id)
            .copied()
            .ok_or(CpuFault::MissingTiming)
    }

    fn label(id: CpuId) -> &'static str {
        match id {
            CpuId::A => "CpuA",
            CpuId::B => "CpuB",
        }
    }

    /// The root run loop. Repeatedly removes the earliest owned event, dispatches it to the
    /// selected child (the borrow of `GlobalClock` ends before the child is stepped), and inserts
    /// any resulting scheduling requests back into the clock. Terminates when the timeline is
    /// exhausted, distinguishing normal completion from deadlock.
    pub fn run(&mut self) -> Result<RunOutcome, CpuFault> {
        // Seed both CPUs to run their first instruction at global tick 0.
        self.clock
            .schedule_at(GlobalTick::ZERO, MachineEvent::Step(CpuId::A))?;
        self.clock
            .schedule_at(GlobalTick::ZERO, MachineEvent::Step(CpuId::B))?;

        while let Some((tick, event)) = self.clock.pop_earliest() {
            match event {
                MachineEvent::Step(id) => self.step_cpu(id, tick)?,
                MachineEvent::Resume {
                    id,
                    continuation,
                    value,
                } => self.resume_receiver(id, continuation, value, tick)?,
            }
            self.drain_wakeups(tick)?;
        }

        // The queue is empty. If any CPU is still parked, no delivery can wake it.
        if self.cpu_a.is_parked() || self.cpu_b.is_parked() {
            Ok(RunOutcome::Deadlock)
        } else {
            Ok(RunOutcome::Completed)
        }
    }

    /// Dispatch one instruction for `id` at global `tick`, attaching instance identity to the
    /// child-local scheduling work it produces.
    fn step_cpu(&mut self, id: CpuId, tick: GlobalTick) -> Result<(), CpuFault> {
        // Obtain an owned instruction; the borrow of program storage ends here.
        let Some(instruction) = self.cpu_ref(id).fetch() else {
            // Program exhausted: the CPU simply drops out of the runnable set.
            return Ok(());
        };

        let (detail, parked_detail) = match instruction {
            RuntimeInstruction::IntegerAdd(_) => ("Add".to_owned(), "Add parks".to_owned()),
            RuntimeInstruction::IntegerSub(_) => ("Sub".to_owned(), "Sub parks".to_owned()),
            RuntimeInstruction::IntegerMul(_) => ("Mul".to_owned(), "Mul parks".to_owned()),
            RuntimeInstruction::Send(send) => (
                format!("send on {:?}", send.channel),
                format!("send parks on {:?}", send.channel),
            ),
            RuntimeInstruction::Recv(recv) => (
                format!("recv on {:?}", recv.channel),
                format!("recv parks on {:?}", recv.channel),
            ),
        };

        let ticks_per_local_cycle = self.ticks_per_local_cycle(id)?;
        let schedule = self.cpu_mut(id).step_at(tick, ticks_per_local_cycle)?;
        self.submit_schedule(id, schedule)?;

        let parked = self.cpu_ref(id).is_parked();
        let detail = if parked { parked_detail } else { detail };
        self.record(tick, id, detail);

        Ok(())
    }

    /// A delivery satisfied a parked receiver: complete its `recv` (push the value, advance past
    /// the `recv`), then schedule its next instruction after its own local duration.
    fn resume_receiver(
        &mut self,
        id: CpuId,
        continuation: ReceiveContinuation,
        value: i64,
        tick: GlobalTick,
    ) -> Result<(), CpuFault> {
        let ticks_per_local_cycle = self.ticks_per_local_cycle(id)?;
        // The child owns continuation completion, including its stack and parked state. The root
        // only supplies the opaque continuation input and resubmits the returned schedule.
        let schedule = self.cpu_mut(id).resume(
            ReceiveCompletion {
                continuation,
                value,
            },
            tick,
            ticks_per_local_cycle,
        )?;
        self.submit_schedule(id, schedule)?;
        self.record(tick, id, format!("wakes, recv {value}"));
        Ok(())
    }

    fn drain_wakeups(&mut self, tick: GlobalTick) -> Result<(), CpuFault> {
        while let Some((continuation, value)) = self.transport.take_wakeup() {
            let id = match continuation.endpoint {
                EndpointId(0) => CpuId::A,
                EndpointId(1) => CpuId::B,
                EndpointId(_) => return Err(CpuFault::UnknownEndpoint),
            };
            let ticks_per_local_cycle = self.ticks_per_local_cycle(id)?;
            let at = self
                .cpu_ref(id)
                .next_boundary_at(tick, ticks_per_local_cycle)?;
            self.clock.schedule_at(
                at,
                MachineEvent::Resume {
                    id,
                    continuation,
                    value,
                },
            )?;
        }
        Ok(())
    }

    /// Map child-local scheduling work to a root event and submit it to the definitive global
    /// clock. The child has already applied its opaque timing policy; the root only attaches `id`.
    fn submit_schedule(
        &mut self,
        id: CpuId,
        schedule: Option<Schedule<CpuEvent>>,
    ) -> Result<(), CpuFault> {
        if let Some(Schedule { at, event }) = schedule {
            match event {
                CpuEvent::RunNext => self.clock.schedule_at(at, MachineEvent::Step(id))?,
            }
        }
        Ok(())
    }

    fn record(&mut self, tick: GlobalTick, id: CpuId, detail: String) {
        self.execution_trace.push(format!(
            "global {:>2}: {} {detail}",
            tick.0,
            Self::label(id)
        ));
    }
}
