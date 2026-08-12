// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use super::{
    arithmetic::{Add, ArithmeticUnit, Mul, Sub, syntax as arithmetic_syntax},
    channel::{
        CHANNEL_A_TO_B, CHANNEL_B_TO_A, ChannelEndpoint, ReceiveCompletion, ReceiveContinuation,
        ReceiveEffect, Recv, Send, SendEffect, SharedTransport, syntax as channel_syntax,
    },
    clock::{
        ClockFault, ClockedComponent, GlobalTick, GlobalTicksPerLocalCycle, LocalCycles, Schedule,
        TimedInstruction,
    },
    debug_trace::DebugTrace,
    execute::Execution,
    handle::{Handle, Observe},
    resume::Resume,
    stack::{Stack, StackFault},
};
use std::convert::From;

#[derive(Clone, Debug, PartialEq)]
pub struct CpuHeader(pub String);

impl vihaco::FromText for CpuHeader {
    fn from_text(text: &str) -> eyre::Result<Self> {
        Ok(Self(text.trim().to_owned()))
    }
}

impl vihaco::SstHeader for CpuHeader {}

vihaco::composite! {
    pub composite Cpu {
        error = CpuFault;

        pub operand_stack: Stack,
        #[syntax("arithmetic")]
        pub alu: ArithmeticUnit,

        #[syntax("channel")]
        pub channel: ChannelEndpoint<i64, SharedTransport<i64>>,

        pub header: Option<CpuHeader>,

        #[device(0x01)]
        #[loadable]
        pub debug: DebugTrace,

        #[program]
        pub program: vihaco::ProgramImage<CpuInstruction, vihaco::NoContext, vihaco::Value, vihaco::Type>,
    }

    syntax {
        header CpuHeader => resolve_header;
    }

    runtime {
        IntegerAdd(Add) => alu {
            message from operand_stack;
            effects {
                observe debug;
                absorb with operand_stack;
            }
        }
        IntegerSub(Sub) => alu {
            message from operand_stack;
            effects {
                observe debug;
                absorb with operand_stack;
            }
        }
        IntegerMul(Mul) => alu {
            message from operand_stack;
            effects {
                observe debug;
                absorb with operand_stack;
            }
        }
        Send(Send) => channel {
            message from operand_stack;
            effects {
                observe debug;
                handle with handle_send;
            }
        }
        Recv(Recv) => channel {
            message none;
            effects {
                observe debug;
                handle with handle_receive;
            }
        }
    }

}

impl From<cpu::syntax::Type> for vihaco::Type {
    fn from(value: cpu::syntax::Type) -> Self {
        match value {
            cpu::syntax::Type::Alu(arithmetic_syntax::ArithmeticType::Integer) => Self::I64,
            cpu::syntax::Type::Channel(channel_syntax::ChannelType::Channel) => Self::U64,
        }
    }
}

impl From<cpu::syntax::Value> for vihaco::Value {
    fn from(value: cpu::syntax::Value) -> Self {
        match value {
            cpu::syntax::Value::Alu(arithmetic_syntax::ArithmeticValue::Zero)
            | cpu::syntax::Value::Channel(channel_syntax::ChannelValue::ToA)
            | cpu::syntax::Value::Channel(channel_syntax::ChannelValue::ToB)
            | cpu::syntax::Value::Channel(channel_syntax::ChannelValue::FromA)
            | cpu::syntax::Value::Channel(channel_syntax::ChannelValue::FromB) => Self::Undefined,
        }
    }
}

impl cpu::syntax::Resolver for Cpu {
    fn resolve_header(&mut self, header: CpuHeader) -> Result<(), CpuFault> {
        self.header = Some(header);
        Ok(())
    }

    fn lower_alu(
        &mut self,
        instruction: arithmetic_syntax::Instruction,
    ) -> Result<Vec<CpuInstruction>, CpuFault> {
        let instruction = match instruction {
            arithmetic_syntax::Instruction::Add => CpuInstruction::IntegerAdd(Add),
            arithmetic_syntax::Instruction::Sub => CpuInstruction::IntegerSub(Sub),
            arithmetic_syntax::Instruction::Mul => CpuInstruction::IntegerMul(Mul),
        };
        Ok(vec![instruction])
    }

    fn lower_channel(
        &mut self,
        instruction: channel_syntax::Instruction,
    ) -> Result<Vec<CpuInstruction>, CpuFault> {
        let (channel, send) = match instruction {
            channel_syntax::Instruction::Send(name) => {
                let channel = match name {
                    channel_syntax::ChannelValue::ToA => CHANNEL_B_TO_A,
                    channel_syntax::ChannelValue::ToB => CHANNEL_A_TO_B,
                    channel_syntax::ChannelValue::FromA => CHANNEL_A_TO_B,
                    channel_syntax::ChannelValue::FromB => CHANNEL_B_TO_A,
                };
                (channel, true)
            }
            channel_syntax::Instruction::Recv(name) => {
                let channel = match name {
                    channel_syntax::ChannelValue::ToA => CHANNEL_B_TO_A,
                    channel_syntax::ChannelValue::ToB => CHANNEL_A_TO_B,
                    channel_syntax::ChannelValue::FromA => CHANNEL_A_TO_B,
                    channel_syntax::ChannelValue::FromB => CHANNEL_B_TO_A,
                };
                (channel, false)
            }
        };
        let instruction = if send {
            CpuInstruction::Send(Send { channel })
        } else {
            CpuInstruction::Recv(Recv { channel })
        };
        Ok(vec![instruction])
    }
}

impl vihaco::loader::LoadSstProgram<vihaco::NoContext> for Cpu {
    fn load_sst_program<'src>(
        &mut self,
        section: vihaco::SstSectionView<'src, vihaco::NoContext>,
    ) -> eyre::Result<()> {
        self.load_source(section)
    }
}

impl Cpu {
    fn handle_send(&mut self, effect: SendEffect) -> Result<(), CpuFault> {
        match effect {}
    }

    fn handle_receive(&mut self, effect: ReceiveEffect<i64>) -> Result<(), CpuFault> {
        match effect {
            ReceiveEffect::Received(value) => self.operand_stack.push(value),
            ReceiveEffect::Parked(_) => {}
        }
        Ok(())
    }
}

impl TimedInstruction for CpuInstruction {
    fn local_cycles(&self) -> LocalCycles {
        match self {
            CpuInstruction::IntegerAdd(_)
            | CpuInstruction::IntegerSub(_)
            | CpuInstruction::IntegerMul(_)
            | CpuInstruction::Send(_)
            | CpuInstruction::Recv(_) => LocalCycles::ONE,
        }
    }
}

impl Cpu {
    pub fn fetch(&self) -> Option<CpuInstruction> {
        self.program
            .module
            .code
            .get(self.program.pc as usize)
            .cloned()
    }

    pub fn finished(&self) -> bool {
        self.program.pc as usize >= self.program.module.code.len() && !self.channel.is_parked()
    }

    pub fn is_parked(&self) -> bool {
        self.channel.is_parked()
    }

    // Resume is intentionally hand-written in phase one. It reuses the generated receive route's
    // observer and handler implementations while leaving continuation ownership to the CPU.
    #[allow(clippy::useless_conversion)]
    fn resume_receive_effects(
        &mut self,
        continuation: ReceiveContinuation,
        value: i64,
    ) -> Result<Execution, CpuFault> {
        let result = self.channel.resume(ReceiveCompletion {
            continuation,
            value,
        })?;
        for effect in result.effects {
            <DebugTrace as Observe<
                ReceiveEffect<i64>,
                __VihacoCpuRoutes::__VihacoRoute_Recv,
            >>::observe(
                &mut self.debug,
                &effect,
            )
            .map_err(Into::<CpuFault>::into)?;
            <Self as Handle<ReceiveEffect<i64>, __VihacoCpuRoutes::__VihacoRoute_Recv>>::handle(
                self, effect,
            )
            .map_err(Into::<CpuFault>::into)?;
        }
        Ok(result.execution)
    }

    /// Finish one instruction, advance the program counter when appropriate, and return owned
    /// child-local scheduling work. This is CPU-internal bookkeeping; the standard clocked
    /// boundary exposes only `step_at` and `resume`.
    fn complete_instruction(
        &mut self,
        global_tick: GlobalTick,
        local_cycles: LocalCycles,
        outcome: Execution,
        ticks_per_local_cycle: GlobalTicksPerLocalCycle,
    ) -> Result<Option<Schedule<CpuEvent>>, CpuFault> {
        if outcome == Execution::Complete {
            self.program.pc += 1;
        }

        if outcome == Execution::Parked {
            return Ok(None);
        }

        let start = self.next_boundary_at(global_tick, ticks_per_local_cycle)?;
        let duration = local_cycles.checked_mul(ticks_per_local_cycle)?;
        let at = start
            .0
            .checked_add(duration.0)
            .map(GlobalTick)
            .ok_or(ClockFault::GlobalTickOverflow)?;
        if self.finished() {
            return Ok(None);
        }
        Ok(Some(Schedule {
            at,
            event: CpuEvent::RunNext,
        }))
    }
}

// ===========================================================================================
// === END GENERATED COMPOSITE SECTION =======================================================
// ===========================================================================================

#[derive(Debug, Clone, Copy)]
pub enum CpuEvent {
    RunNext,
}

impl ClockedComponent for Cpu {
    type Event = CpuEvent;
    type Completion = ReceiveCompletion<i64>;
    type Fault = CpuFault;

    fn step_at(
        &mut self,
        global_tick: GlobalTick,
        ticks_per_local_cycle: GlobalTicksPerLocalCycle,
    ) -> Result<Option<Schedule<CpuEvent>>, CpuFault> {
        let Some(instruction) = self.fetch() else {
            return Ok(None);
        };
        let local_cycles = instruction.local_cycles();
        let execution = self.execute_generated(&instruction)?;
        self.complete_instruction(global_tick, local_cycles, execution, ticks_per_local_cycle)
    }

    fn resume(
        &mut self,
        completion: ReceiveCompletion<i64>,
        global_tick: GlobalTick,
        ticks_per_local_cycle: GlobalTicksPerLocalCycle,
    ) -> Result<Option<Schedule<CpuEvent>>, CpuFault> {
        let outcome = self.resume_receive_effects(completion.continuation, completion.value)?;
        let instruction = self.fetch().ok_or(CpuFault::MissingInstruction)?;
        let local_cycles = instruction.local_cycles();
        self.complete_instruction(global_tick, local_cycles, outcome, ticks_per_local_cycle)
    }

    fn next_boundary_at(
        &self,
        global_tick: GlobalTick,
        ticks_per_local_cycle: GlobalTicksPerLocalCycle,
    ) -> Result<GlobalTick, CpuFault> {
        let ratio = ticks_per_local_cycle.0;
        let cycles = global_tick
            .0
            .checked_add(ratio - 1)
            .ok_or(CpuFault::Clock(ClockFault::GlobalTickOverflow))?
            / ratio;
        cycles
            .checked_mul(ratio)
            .map(GlobalTick)
            .ok_or(CpuFault::Clock(ClockFault::GlobalTickOverflow))
    }
}

// ===========================================================================================
// === GENERATED ERROR PLUMBING ===============================================================
// ===========================================================================================
//
// The composite macro supplies the route error conversions so generated pipeline code can use
// `?` across component boundaries.

/// Machine-level fault, with the `From` conversions generated for route plumbing.
#[derive(Debug)]
pub enum CpuFault {
    Stack(StackFault),
    Clock(ClockFault),
    Loading(eyre::Report),
    UnknownEndpoint,
    MissingInstruction,
    MissingTiming,
}

impl std::fmt::Display for CpuFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CpuFault {}

impl From<StackFault> for CpuFault {
    fn from(fault: StackFault) -> Self {
        CpuFault::Stack(fault)
    }
}

impl From<std::convert::Infallible> for CpuFault {
    fn from(never: std::convert::Infallible) -> Self {
        match never {}
    }
}

impl From<ClockFault> for CpuFault {
    fn from(fault: ClockFault) -> Self {
        CpuFault::Clock(fault)
    }
}

impl From<eyre::Report> for CpuFault {
    fn from(fault: eyre::Report) -> Self {
        CpuFault::Loading(fault)
    }
}

// ===========================================================================================
// === END GENERATED ERROR PLUMBING ===========================================================
// ===========================================================================================
