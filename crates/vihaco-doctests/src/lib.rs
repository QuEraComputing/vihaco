//! Doc tests for the vihaco documentation site.
//!
//! This crate ships no real code. It exists so CI fails if the documentation
//! drifts from the actual API:
//!
//! - The example files shown on the landing and quick-start pages
//!   (`docs/examples/*.rs`) are `include!`d below, so they are compiled exactly
//!   as the site shows them (and the runnable ones are executed by `#[test]`).
//! - Every fenced ` ```rust ` block in the guide markdown is a rustdoc doctest;
//!   run them with `cargo test --doc -p vihaco-doctests`. Blocks that are
//!   genuinely illustrative (trait reproductions, runtime pseudocode that calls
//!   helpers a real consumer would write) are marked `ignore`; blocks that
//!   compile but should not run are marked `no_run`.
//!
//! The example files define `pub` items that nothing else uses, hence the
//! crate-wide `dead_code` allowance.
#![allow(dead_code, unused_imports, unused_variables)]

// ── Landing / quick-start example files (compiled as shown on the site) ──

mod ex_counter {
    include!("../../../docs/examples/counter.rs");
}

mod ex_observe {
    include!("../../../docs/examples/observe.rs");
}

mod ex_quickstart {
    include!("../../../docs/examples/quickstart.rs");

    #[test]
    fn runs() {
        main().expect("quickstart example should run");
    }
}

mod ex_quickstart_parse {
    include!("../../../docs/examples/quickstart_parse.rs");

    #[test]
    fn runs() {
        main();
    }
}

// ── Guide tutorials (every ```rust block is a rustdoc doctest) ──

#[doc = include_str!("../../../docs/src/pages/guide/instructions.md")]
mod guide_instructions {}

#[doc = include_str!("../../../docs/src/pages/guide/instructions-advanced.md")]
mod guide_instructions_advanced {}

#[doc = include_str!("../../../docs/src/pages/guide/parser.md")]
mod guide_parser {}

#[doc = include_str!("../../../docs/src/pages/guide/parser-advanced.md")]
mod guide_parser_advanced {}

#[doc = include_str!("../../../docs/src/pages/guide/messages.md")]
mod guide_messages {}

#[doc = include_str!("../../../docs/src/pages/guide/components.md")]
mod guide_components {}

#[doc = include_str!("../../../docs/src/pages/guide/observers.md")]
mod guide_observers {}

#[doc = include_str!("../../../docs/src/pages/guide/composites.md")]
mod guide_composites {}
