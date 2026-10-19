// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, cpu};
use vihaco_runtime::{Execute, PopFromStack, PushToStack};

impl Execute<cpu::NegI32> for CPU {
    type Message = PopFromStack<(i32,)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NegI32,
        PopFromStack((operand,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = operand.wrapping_neg();
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::NegI64> for CPU {
    type Message = PopFromStack<(i64,)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NegI64,
        PopFromStack((operand,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = operand.wrapping_neg();
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::NegF32> for CPU {
    type Message = PopFromStack<(f32,)>;
    type Effect = PushToStack<(f32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NegF32,
        PopFromStack((operand,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = -operand;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::NegF64> for CPU {
    type Message = PopFromStack<(f64,)>;
    type Effect = PushToStack<(f64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NegF64,
        PopFromStack((operand,)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = -operand;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::EqI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::EqI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs == rhs,)))
    }
}

impl Execute<cpu::EqI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::EqI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs == rhs,)))
    }
}

impl Execute<cpu::EqU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::EqU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs == rhs,)))
    }
}

impl Execute<cpu::EqU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::EqU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs == rhs,)))
    }
}

impl Execute<cpu::EqF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::EqF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs == rhs,)))
    }
}

impl Execute<cpu::EqF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::EqF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs == rhs,)))
    }
}

impl Execute<cpu::NeI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NeI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs != rhs,)))
    }
}

impl Execute<cpu::NeI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NeI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs != rhs,)))
    }
}

impl Execute<cpu::NeU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NeU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs != rhs,)))
    }
}

impl Execute<cpu::NeU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NeU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs != rhs,)))
    }
}

impl Execute<cpu::NeF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NeF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs != rhs,)))
    }
}

impl Execute<cpu::NeF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::NeF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs != rhs,)))
    }
}

impl Execute<cpu::LtI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LtI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs < rhs,)))
    }
}

impl Execute<cpu::LtI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LtI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs < rhs,)))
    }
}

impl Execute<cpu::LtU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LtU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs < rhs,)))
    }
}

impl Execute<cpu::LtU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LtU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs < rhs,)))
    }
}

impl Execute<cpu::LtF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LtF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs < rhs,)))
    }
}

impl Execute<cpu::LtF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LtF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs < rhs,)))
    }
}

impl Execute<cpu::GtI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GtI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs > rhs,)))
    }
}

impl Execute<cpu::GtI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GtI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs > rhs,)))
    }
}

impl Execute<cpu::GtU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GtU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs > rhs,)))
    }
}

impl Execute<cpu::GtU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GtU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs > rhs,)))
    }
}

impl Execute<cpu::GtF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GtF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs > rhs,)))
    }
}

impl Execute<cpu::GtF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GtF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs > rhs,)))
    }
}

impl Execute<cpu::LeI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LeI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs <= rhs,)))
    }
}

impl Execute<cpu::LeI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LeI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs <= rhs,)))
    }
}

impl Execute<cpu::LeU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LeU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs <= rhs,)))
    }
}

impl Execute<cpu::LeU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LeU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs <= rhs,)))
    }
}

impl Execute<cpu::LeF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LeF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs <= rhs,)))
    }
}

impl Execute<cpu::LeF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::LeF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs <= rhs,)))
    }
}

impl Execute<cpu::GeI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GeI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs >= rhs,)))
    }
}

impl Execute<cpu::GeI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GeI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs >= rhs,)))
    }
}

impl Execute<cpu::GeU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GeU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs >= rhs,)))
    }
}

impl Execute<cpu::GeU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GeU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs >= rhs,)))
    }
}

impl Execute<cpu::GeF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GeF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs >= rhs,)))
    }
}

impl Execute<cpu::GeF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(bool,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::GeF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs >= rhs,)))
    }
}
