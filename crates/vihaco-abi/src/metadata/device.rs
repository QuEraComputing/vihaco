// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceMetadata {
    pub code: u8,
    pub name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSymbolAliasMetadata {
    pub name: &'static str,
    pub device_code: u8,
}
