// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Doc tests for the vihaco documentation site.
//!
//! This crate ships no real code. It exists so CI fails if the documentation
//! drifts from the actual API:
//!
//! - The example files shown on the landing and quick-start pages
//!   (`docs/examples/*.rs`) are `include!`d below, so they are compiled exactly
//!   as the site shows them (and the runnable ones are executed by `#[test]`).
//! - Every fenced ` ```rust ` block in the guide markdown is a rustdoc doctest;
//!   run them with `cargo test --doc -p vihaco-doctests --features program-image-execution`.
//!   Blocks that are
//!   genuinely illustrative (trait reproductions, runtime pseudocode that calls
//!   helpers a real consumer would write) are marked `ignore`; blocks that
//!   compile but should not run are marked `no_run`.
//!
//! The example files define `pub` items that nothing else uses, hence the
//! crate-wide `dead_code` allowance.
#![allow(dead_code, unused_imports, unused_variables)]

// ── Landing / quick-start example files (compiled as shown on the site) ──

#[cfg(feature = "program-image-execution")]
mod ex_counter {
    include!("../../../docs/examples/counter.rs");
}

#[cfg(feature = "program-image-execution")]
mod ex_quickstart {
    include!("../../../docs/examples/quickstart.rs");

    #[test]
    fn runs() {
        main().expect("quickstart example should run");
    }
}

#[cfg(feature = "program-image-execution")]
mod ex_patterns {
    include!("../../../docs/examples/patterns.rs");

    #[test]
    fn runs() {
        main().expect("pattern examples should run");
    }
}

#[cfg(feature = "program-image-execution")]
mod ex_quickstart_parse {
    include!("../../../docs/examples/quickstart_parse.rs");

    #[test]
    fn runs() {
        main();
    }
}

// ── Guide tutorials (every ```rust block is a rustdoc doctest) ──

#[cfg(feature = "program-image-execution")]
mod authors {
    #[doc = include_str!("../../../docs/src/pages/guide/mental-model.md")]
    mod mental_model {}

    #[doc = include_str!("../../../docs/src/pages/guide/first-machine.md")]
    mod first_machine {}

    #[doc = include_str!("../../../docs/src/pages/guide/execution.md")]
    mod execution {}

    #[doc = include_str!("../../../docs/src/pages/guide/stack.md")]
    mod stack {}

    #[doc = include_str!("../../../docs/src/pages/guide/program-execution.md")]
    mod program_execution {}

    #[doc = include_str!("../../../docs/src/pages/guide/loadables.md")]
    mod loadables {}

    #[doc = include_str!("../../../docs/src/pages/guide/patterns.md")]
    mod patterns {}

    #[doc = include_str!("../../../docs/src/pages/guide/testing.md")]
    mod testing {}

    #[doc = include_str!("../../../docs/src/pages/guide/instructions.md")]
    mod guide_instructions {}

    #[doc = include_str!("../../../docs/src/pages/guide/instructions-advanced.md")]
    mod guide_instructions_advanced {}

    #[doc = include_str!("../../../docs/src/pages/guide/parser.md")]
    mod guide_parser {}

    #[doc = include_str!("../../../docs/src/pages/guide/parser-patterns.md")]
    mod guide_parser_patterns {}

    #[doc = include_str!("../../../docs/src/pages/guide/parser-advanced.md")]
    mod guide_parser_advanced {}

    #[doc = include_str!("../../../docs/src/pages/guide/messages.md")]
    mod guide_messages {}

    #[doc = include_str!("../../../docs/src/pages/guide/components.md")]
    mod guide_components {}

    #[doc = include_str!("../../../docs/src/pages/guide/effects.md")]
    mod guide_effects {}

    #[doc = include_str!("../../../docs/src/pages/guide/composites.md")]
    mod guide_composites {}
}
