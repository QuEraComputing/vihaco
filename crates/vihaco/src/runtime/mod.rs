// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

mod generated;
mod marker;
mod observe;

pub use crate::Effects;
pub use crate::traits::EffectSink;
pub use generated::{CompositeMetadata, GeneratedComponent, expect_exactly_one_effect};
pub use marker::Message;
pub use observe::Observe;
