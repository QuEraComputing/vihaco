// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::word::*;
use crate::{CPU, StepOutcome, cpu};
use vihaco::Execute;
use vihaco::traits::StackMemory;

impl Execute<cpu::NegI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NegI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let value = decode_i32(self.stack_pop()?).wrapping_neg();
        self.stack_push(encode_i32(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NegI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NegI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let value = decode_i64(self.stack_pop()?).wrapping_neg();
        self.stack_push(encode_i64(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NegF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NegF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let value = -decode_f32(self.stack_pop()?);
        self.stack_push(encode_f32(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NegF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NegF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let value = -decode_f64(self.stack_pop()?);
        self.stack_push(encode_f64(value));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::EqI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::EqI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs == rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::EqI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::EqI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs == rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::EqU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::EqU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs == rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::EqU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::EqU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs == rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::EqF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::EqF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs == rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::EqF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::EqF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs == rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NeI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NeI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs != rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NeI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NeI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs != rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NeU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NeU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs != rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NeU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NeU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs != rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NeF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NeF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs != rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::NeF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::NeF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs != rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LtI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LtI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs < rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LtI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LtI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs < rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LtU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LtU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs < rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LtU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LtU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs < rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LtF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LtF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs < rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LtF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LtF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs < rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GtI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GtI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs > rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GtI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GtI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs > rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GtU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GtU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs > rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GtU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GtU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs > rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GtF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GtF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs > rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GtF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GtF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs > rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LeI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LeI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs <= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LeI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LeI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs <= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LeU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LeU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs <= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LeU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LeU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs <= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LeF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LeF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs <= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::LeF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::LeF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs <= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GeI32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GeI32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i32(self.stack_pop()?);
        let lhs = decode_i32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs >= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GeI64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GeI64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_i64(self.stack_pop()?);
        let lhs = decode_i64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs >= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GeU32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GeU32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u32(self.stack_pop()?);
        let lhs = decode_u32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs >= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GeU64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GeU64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_u64(self.stack_pop()?);
        let lhs = decode_u64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs >= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GeF32> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GeF32, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f32(self.stack_pop()?);
        let lhs = decode_f32(self.stack_pop()?);
        self.stack_push(encode_bool(lhs >= rhs));
        Ok(StepOutcome::Continue)
    }
}

impl Execute<cpu::GeF64> for CPU {
    type Message = ();
    type Effect = StepOutcome;
    type Error = eyre::Report;

    fn execute(&mut self, _instruction: &cpu::GeF64, _: ()) -> eyre::Result<StepOutcome> {
        self.clear_pending_pc();
        let rhs = decode_f64(self.stack_pop()?);
        let lhs = decode_f64(self.stack_pop()?);
        self.stack_push(encode_bool(lhs >= rhs));
        Ok(StepOutcome::Continue)
    }
}
