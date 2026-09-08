// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use crate::SurfaceInstruction;
pub use vihaco::{DeviceInstruction, LocalCountsByDevice};

/// Compute each CPU device's local slot requirement for a function body.
///
/// Parameters occupy the first `arity` slots. Every load/store reference counts,
/// including references in unreachable instructions. A device with no CPU
/// instructions has no entry: its exact requirement is `arity` slots.
/// Composite surface instructions convert by reference, so the body can still
/// be used for lowering after analysis.
///
/// # Errors
/// Returns an error if a local index plus one cannot be represented as `u32`.
pub fn required_local_count<'a, const DEVICE_CODE: u8, T>(
    arity: u32,
    instructions: impl IntoIterator<Item = T>,
) -> eyre::Result<u32>
where
    T: Into<Option<DeviceInstruction<'a, SurfaceInstruction, DEVICE_CODE>>>,
{
    use SurfaceInstruction::*;
    let mut count = arity;
    for instruction in instructions {
        let Some(DeviceInstruction { instruction }) = instruction.into() else {
            continue;
        };
        let index = match instruction {
            LoadI32(i) | LoadI64(i) | LoadU32(i) | LoadU64(i) | LoadF32(i) | LoadF64(i)
            | LoadBool(i) | StoreI32(i) | StoreI64(i) | StoreU32(i) | StoreU64(i) | StoreF32(i)
            | StoreF64(i) | StoreBool(i) => *i,
            _ => continue,
        };
        let required = index.checked_add(1).ok_or_else(|| {
            eyre::eyre!("local index too large for device {DEVICE_CODE}: {index}")
        })?;
        count = count.max(required);
    }
    Ok(count)
}
