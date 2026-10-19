// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::{CPU, cpu};
use vihaco_runtime::{Execute, PopFromStack, PushToStack};

impl Execute<cpu::ShlI32> for CPU {
    type Message = PopFromStack<(i32, u32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::ShlI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_shl(rhs & 31),)))
    }
}

impl Execute<cpu::ShlI64> for CPU {
    type Message = PopFromStack<(i64, u32)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::ShlI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_shl(rhs & 63),)))
    }
}

impl Execute<cpu::ShlU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::ShlU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_shl(rhs & 31),)))
    }
}

impl Execute<cpu::ShlU64> for CPU {
    type Message = PopFromStack<(u64, u32)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::ShlU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_shl(rhs & 63),)))
    }
}

impl Execute<cpu::ShrI32> for CPU {
    type Message = PopFromStack<(i32, u32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::ShrI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_shr(rhs & 31),)))
    }
}

impl Execute<cpu::ShrI64> for CPU {
    type Message = PopFromStack<(i64, u32)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::ShrI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_shr(rhs & 63),)))
    }
}

impl Execute<cpu::ShrU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::ShrU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_shr(rhs & 31),)))
    }
}

impl Execute<cpu::ShrU64> for CPU {
    type Message = PopFromStack<(u64, u32)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::ShrU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.wrapping_shr(rhs & 63),)))
    }
}

impl Execute<cpu::RolI32> for CPU {
    type Message = PopFromStack<(i32, u32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RolI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.rotate_left(rhs),)))
    }
}

impl Execute<cpu::RolI64> for CPU {
    type Message = PopFromStack<(i64, u32)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RolI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.rotate_left(rhs),)))
    }
}

impl Execute<cpu::RolU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RolU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.rotate_left(rhs),)))
    }
}

impl Execute<cpu::RolU64> for CPU {
    type Message = PopFromStack<(u64, u32)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RolU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.rotate_left(rhs),)))
    }
}

impl Execute<cpu::RorI32> for CPU {
    type Message = PopFromStack<(i32, u32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RorI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.rotate_right(rhs),)))
    }
}

impl Execute<cpu::RorI64> for CPU {
    type Message = PopFromStack<(i64, u32)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RorI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.rotate_right(rhs),)))
    }
}

impl Execute<cpu::RorU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RorU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.rotate_right(rhs),)))
    }
}

impl Execute<cpu::RorU64> for CPU {
    type Message = PopFromStack<(u64, u32)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::RorU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs.rotate_right(rhs),)))
    }
}

impl Execute<cpu::BitAndI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitAndI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs & rhs,)))
    }
}

impl Execute<cpu::BitAndI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitAndI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs & rhs,)))
    }
}

impl Execute<cpu::BitAndU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitAndU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs & rhs,)))
    }
}

impl Execute<cpu::BitAndU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitAndU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs & rhs,)))
    }
}

impl Execute<cpu::BitOrI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitOrI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs | rhs,)))
    }
}

impl Execute<cpu::BitOrI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitOrI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs | rhs,)))
    }
}

impl Execute<cpu::BitOrU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitOrU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs | rhs,)))
    }
}

impl Execute<cpu::BitOrU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitOrU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs | rhs,)))
    }
}

impl Execute<cpu::BitXorI32> for CPU {
    type Message = PopFromStack<(i32, i32)>;
    type Effect = PushToStack<(i32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitXorI32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs ^ rhs,)))
    }
}

impl Execute<cpu::BitXorI64> for CPU {
    type Message = PopFromStack<(i64, i64)>;
    type Effect = PushToStack<(i64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitXorI64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs ^ rhs,)))
    }
}

impl Execute<cpu::BitXorU32> for CPU {
    type Message = PopFromStack<(u32, u32)>;
    type Effect = PushToStack<(u32,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitXorU32,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs ^ rhs,)))
    }
}

impl Execute<cpu::BitXorU64> for CPU {
    type Message = PopFromStack<(u64, u64)>;
    type Effect = PushToStack<(u64,)>;
    type Error = eyre::Report;

    fn execute(
        &mut self,
        _: &cpu::BitXorU64,
        PopFromStack((lhs, rhs)): Self::Message,
    ) -> eyre::Result<Self::Effect> {
        Ok(PushToStack((lhs ^ rhs,)))
    }
}
