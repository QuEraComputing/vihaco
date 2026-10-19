// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, cpu};
use vihaco_runtime::{Execute, PopFromStack, PushToStack};

impl Execute<cpu::AddI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::AddI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_add(rhs),)))
    }
}

impl Execute<cpu::AddF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(f32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::AddF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs + rhs,)))
    }
}

impl Execute<cpu::AddI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::AddI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_add(rhs),)))
    }
}

impl Execute<cpu::AddU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::AddU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_add(rhs),)))
    }
}

impl Execute<cpu::AddU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::AddU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_add(rhs),)))
    }
}

impl Execute<cpu::AddF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(f64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::AddF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs + rhs,)))
    }
}

impl Execute<cpu::SubI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::SubI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_sub(rhs),)))
    }
}

impl Execute<cpu::SubI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::SubI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_sub(rhs),)))
    }
}

impl Execute<cpu::SubU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::SubU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_sub(rhs),)))
    }
}

impl Execute<cpu::SubU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::SubU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_sub(rhs),)))
    }
}

impl Execute<cpu::SubF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(f32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::SubF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs - rhs,)))
    }
}

impl Execute<cpu::SubF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(f64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::SubF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs - rhs,)))
    }
}

impl Execute<cpu::MulI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::MulI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_mul(rhs),)))
    }
}

impl Execute<cpu::MulI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::MulI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_mul(rhs),)))
    }
}

impl Execute<cpu::MulU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::MulU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_mul(rhs),)))
    }
}

impl Execute<cpu::MulU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::MulU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_mul(rhs),)))
    }
}

impl Execute<cpu::MulF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(f32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::MulF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs * rhs,)))
    }
}

impl Execute<cpu::MulF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(f64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::MulF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs * rhs,)))
    }
}

impl Execute<cpu::DivI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::DivI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = lhs
            .checked_div(rhs)
            .ok_or_else(|| eyre::eyre!("integer division error"))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::DivI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::DivI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = lhs
            .checked_div(rhs)
            .ok_or_else(|| eyre::eyre!("integer division error"))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::DivU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::DivU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = lhs
            .checked_div(rhs)
            .ok_or_else(|| eyre::eyre!("integer division error"))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::DivU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::DivU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = lhs
            .checked_div(rhs)
            .ok_or_else(|| eyre::eyre!("integer division error"))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::DivF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(f32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::DivF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs / rhs,)))
    }
}

impl Execute<cpu::DivF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(f64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::DivF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs / rhs,)))
    }
}

impl Execute<cpu::RemI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RemI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = lhs
            .checked_rem(rhs)
            .ok_or_else(|| eyre::eyre!("integer remainder error"))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::RemI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RemI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = lhs
            .checked_rem(rhs)
            .ok_or_else(|| eyre::eyre!("integer remainder error"))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::RemU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RemU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = lhs
            .checked_rem(rhs)
            .ok_or_else(|| eyre::eyre!("integer remainder error"))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::RemU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RemU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        let value = lhs
            .checked_rem(rhs)
            .ok_or_else(|| eyre::eyre!("integer remainder error"))?;
        Ok(PushToStack((value,)))
    }
}

impl Execute<cpu::RemF32> for CPU {
    type Message = PopFromStack<(f32, f32)>;
    type Effect = PushToStack<(f32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RemF32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs % rhs,)))
    }
}

impl Execute<cpu::RemF64> for CPU {
    type Message = PopFromStack<(f64, f64)>;
    type Effect = PushToStack<(f64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RemF64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs % rhs,)))
    }
}
