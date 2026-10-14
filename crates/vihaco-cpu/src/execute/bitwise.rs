// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::word::*;
use crate::{CPU, StepOutcome, cpu};
use vihaco::Execute;
use vihaco::traits::StackMemory;

impl Execute<cpu::ShlI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ShlI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs.wrapping_shl(rhs & 31)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ShlI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ShlI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs.wrapping_shl(rhs & 63)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ShlU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ShlU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs.wrapping_shl(rhs & 31)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ShlU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ShlU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs.wrapping_shl(rhs & 63)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ShrI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ShrI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs.wrapping_shr(rhs & 31)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ShrI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ShrI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs.wrapping_shr(rhs & 63)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ShrU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ShrU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs.wrapping_shr(rhs & 31)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::ShrU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::ShrU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs.wrapping_shr(rhs & 63)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RolI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RolI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs.rotate_left(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RolI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RolI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs.rotate_left(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RolU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RolU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs.rotate_left(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RolU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RolU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs.rotate_left(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RorI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RorI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs.rotate_right(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RorI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RorI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs.rotate_right(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RorU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RorU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs.rotate_right(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RorU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RorU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs.rotate_right(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitAndI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitAndI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs & rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitAndI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitAndI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs & rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitAndU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitAndU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs & rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitAndU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitAndU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs & rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitOrI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitOrI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs | rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitOrI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitOrI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs | rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitOrU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitOrU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs | rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitOrU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitOrU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs | rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitXorI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitXorI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs ^ rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitXorI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitXorI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs ^ rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitXorU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitXorU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs ^ rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::BitXorU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::BitXorU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs ^ rhs));
        Ok(StepOutcome::Continue)
    }
}
