// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Program-construction capabilities and legacy section-loading hooks.
//!
//! With `program-image-execution`, callers construct images explicitly and the
//! section-loading hooks are unavailable. Lowering capabilities remain available
//! in both feature configurations.

#[cfg(not(feature = "program-image-execution"))]
use vihaco_bytecode::{BytecodeSectionView, SSTSectionView};

pub use crate::lowering::MetadataLoweringContext;

/// Allow a machine to load one bytecode section.
///
/// Sections form a flat table; pick one with [`BytecodeFile::section`] and pass
/// its view here. Return program data to an outer owner rather than storing it
/// inside Dispatch. Loaders that only mutate their receiver use
/// `type Output = ();`.
///
/// Loading is not transactional. On error, no output is returned, but earlier
/// mutations to this loader are retained.
///
/// [`BytecodeFile::section`]: vihaco_bytecode::BytecodeFile::section
#[cfg(not(feature = "program-image-execution"))]
pub trait LoadBytecodeSection<C> {
    /// The artifact produced for this section, or `()` for a loader that stores it.
    type Output;

    fn load_bytecode_section<'bc>(
        &mut self,
        section: BytecodeSectionView<'bc, C>,
    ) -> eyre::Result<Self::Output>;
}

/// Allow a machine to load one SST section.
///
/// Sections form a flat list; pick one with [`SSTFile::section`] and pass its
/// view here. Return program data to an outer owner rather than storing it
/// inside Dispatch. Loaders that only mutate their receiver use
/// `type Output = ();`.
///
/// Loading is not transactional. On error, no output is returned, but earlier
/// mutations to this loader are retained.
///
/// [`SSTFile::section`]: vihaco_bytecode::SSTFile::section
#[cfg(not(feature = "program-image-execution"))]
pub trait LoadSSTSection<C> {
    /// The artifact produced for this section, or `()` for a loader that stores it.
    type Output;

    fn load_sst_section<'bc>(
        &mut self,
        section: SSTSectionView<'bc, C>,
    ) -> eyre::Result<Self::Output>;

    /// Lower a section's parsed instructions after preparing loader capabilities.
    ///
    /// Call this from `load_sst_section` before storing executable code.
    /// For a generated composite syntax aggregate, the single `Lower<Self>`
    /// bound requires every included instruction's loader capabilities. An
    /// error returns no partial instruction vector.
    fn lower_instructions<I>(
        &mut self,
        instructions: impl IntoIterator<Item = I>,
    ) -> eyre::Result<Vec<I::Runtime>>
    where
        I: vihaco_abi::Lower<Self>,
    {
        instructions
            .into_iter()
            .map(|instruction| instruction.lower(self))
            .collect()
    }
}
