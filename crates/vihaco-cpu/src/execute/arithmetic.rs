// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::word::*;
use crate::{CPU, StepOutcome, cpu};
use vihaco::Execute;
use vihaco::traits::StackMemory;

impl Execute<cpu::AddI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::AddI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs.wrapping_add(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::AddF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::AddF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_f32(lhs + rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::AddI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::AddI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs.wrapping_add(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::AddU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::AddU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs.wrapping_add(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::AddU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::AddU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs.wrapping_add(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::AddF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::AddF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_f64(lhs + rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::SubI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::SubI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs.wrapping_sub(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::SubI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::SubI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs.wrapping_sub(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::SubU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::SubU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs.wrapping_sub(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::SubU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::SubU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs.wrapping_sub(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::SubF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::SubF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_f32(lhs - rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::SubF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::SubF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_f64(lhs - rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::MulI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::MulI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_i32(lhs.wrapping_mul(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::MulI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::MulI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_i64(lhs.wrapping_mul(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::MulU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::MulU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_u32(lhs.wrapping_mul(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::MulU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::MulU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_u64(lhs.wrapping_mul(rhs)));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::MulF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::MulF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_f32(lhs * rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::MulF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::MulF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_f64(lhs * rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::DivI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::DivI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        let value = lhs
            .checked_div(rhs)
            .ok_or_else(|| eyre::eyre!("integer division error"))?;
        self.stack_push(encode_i32(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::DivI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::DivI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        let value = lhs
            .checked_div(rhs)
            .ok_or_else(|| eyre::eyre!("integer division error"))?;
        self.stack_push(encode_i64(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::DivU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::DivU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        let value = lhs
            .checked_div(rhs)
            .ok_or_else(|| eyre::eyre!("integer division error"))?;
        self.stack_push(encode_u32(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::DivU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::DivU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        let value = lhs
            .checked_div(rhs)
            .ok_or_else(|| eyre::eyre!("integer division error"))?;
        self.stack_push(encode_u64(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::DivF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::DivF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_f32(lhs / rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::DivF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::DivF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_f64(lhs / rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RemI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RemI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        let value = lhs
            .checked_rem(rhs)
            .ok_or_else(|| eyre::eyre!("integer remainder error"))?;
        self.stack_push(encode_i32(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RemI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RemI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        let value = lhs
            .checked_rem(rhs)
            .ok_or_else(|| eyre::eyre!("integer remainder error"))?;
        self.stack_push(encode_i64(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RemU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RemU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        let value = lhs
            .checked_rem(rhs)
            .ok_or_else(|| eyre::eyre!("integer remainder error"))?;
        self.stack_push(encode_u32(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RemU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RemU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        let value = lhs
            .checked_rem(rhs)
            .ok_or_else(|| eyre::eyre!("integer remainder error"))?;
        self.stack_push(encode_u64(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RemF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RemF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_f32(lhs % rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::RemF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::RemF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_f64(lhs % rhs));
        Ok(StepOutcome::Continue)
    }
}
