// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// A borrowed instruction and its device's code in the containing composite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceInstruction<'a, I, const DEVICE_CODE: u8> {
    pub instruction: &'a I,
}
