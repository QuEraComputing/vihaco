// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

use std::iter::FusedIterator;

use crate::__private::{Capabilities, Description};

/// Internal report row for an instruction at its canonical device destination.
///
/// Capability order follows device fields, dialect lists, and instruction
/// declarations. Availability is independent of executable-set selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionCapability {
    /// Canonical device fields, from the outermost composite to the component.
    /// Empty when reporting a component directly.
    pub target: Vec<&'static str>,
    /// Canonical dialect name, unaffected by Rust import aliases.
    pub dialect: &'static str,
    /// Canonical instruction mnemonic.
    pub instruction: &'static str,
}

/// Internal reporting of a component or composite's validated capabilities.
///
/// Generated support under `__private`, not a user-facing API. Available with
/// `typed-declarations`. The declaration macros supply the
/// hidden bound; manual description implementations are unsupported.
/// Instruction sets do not filter this report, and source aliases do not
/// duplicate entries. Traversal is lazy and retains declaration order.
///
/// Invalid concrete descriptions fail const evaluation when this function is
/// instantiated during code generation. `cargo check` alone may not evaluate
/// generic reporting calls; owning macros also force validation at fixed
/// declarations and concrete instruction-set associations.
pub fn capabilities<C: Capabilities>() -> impl FusedIterator<Item = InstructionCapability> {
    const { crate::__private::validate(C::DESCRIPTION) };
    CapabilityIter {
        stack: vec![(C::DESCRIPTION, 0, 0)],
        target: Vec::new(),
    }
}

struct CapabilityIter {
    // Component frames index dialect/leaf; composite frames index next device.
    stack: Vec<(&'static Description, usize, usize)>,
    target: Vec<&'static str>,
}

impl Iterator for CapabilityIter {
    type Item = InstructionCapability;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((description, index, leaf_index)) = self.stack.last_mut() {
            match description {
                Description::Component(dialects) => {
                    if let Some(dialect) = dialects.get(*index) {
                        if let Some(leaf) = dialect.get(*leaf_index) {
                            *leaf_index += 1;
                            return Some(InstructionCapability {
                                target: self.target.clone(),
                                dialect: leaf.dialect,
                                instruction: leaf.mnemonic,
                            });
                        }
                        *index += 1;
                        *leaf_index = 0;
                        continue;
                    }
                }
                Description::Composite(devices) => {
                    if let Some(device) = devices.get(*index) {
                        *index += 1;
                        self.target.push(device.name);
                        self.stack.push((device.child, 0, 0));
                        continue;
                    }
                }
            }
            self.stack.pop();
            if !self.stack.is_empty() {
                self.target.pop();
            }
        }
        None
    }
}

impl FusedIterator for CapabilityIter {}
