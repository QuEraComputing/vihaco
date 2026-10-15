//! Step 1 mechanism model, not a production runtime API.

pub use chumsky;
pub use eyre;
pub use vihaco_parser as vihaco_parser_ident;
pub use vihaco_parser::{Parse, SurfaceInstruction};
pub use vihaco_parser_derive::Parse;
pub use vihaco_runtime::{Effects, HasInstructionSet};

#[cfg(feature = "typed-declarations")]
pub use typed::*;

#[cfg(feature = "typed-declarations")]
mod typed {
    use super::*;

    // The experimental component marker needs no aggregate instruction set.
    pub trait Component {}
    pub trait Execute<I>: Component {
        type Message;
        type Effect;
        type Error;
        fn execute(
            &mut self,
            instruction: &I,
            message: Self::Message,
        ) -> Result<Self::Effect, Self::Error>;
    }
    pub trait InstructionSpec {
        type Surface: for<'src> Parse<'src>;
        type Dialect: DialectSpec;
        const MNEMONIC: &'static str;
    }
    pub trait DialectSpec {
        const LEAVES: &'static [Leaf];
    }
    pub trait Supports<D: DialectSpec>: Component {}
    pub trait Coverage<C: Component>: DialectSpec {}
    pub fn require_coverage<C: Component, D: Coverage<C>>() {}
    pub fn require_available<C, I>()
    where
        I: InstructionSpec,
        C: Supports<I::Dialect> + Execute<I>,
    {
    }

    pub trait Field<const INDEX: usize> {
        type Child;
        const SOURCE: &'static [&'static str];
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Leaf {
        pub head: &'static str,
        pub name: &'static str,
    }
    pub enum Description {
        Component(&'static [&'static [Leaf]]),
        Composite(&'static [Device]),
    }
    pub struct Device {
        pub name: &'static str,
        pub child: &'static Description,
    }
    pub trait Capabilities {
        const DESCRIPTION: &'static Description;
    }

    const fn equal(a: &str, b: &str) -> bool {
        let a = a.as_bytes();
        let b = b.as_bytes();
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }
    // Each generated dialect list has one canonical dialect name. Different names
    // cannot overlap; only matching names require instruction-pair comparisons.
    const fn assert_disjoint_dialects(mut left: &[Leaf], right: &[Leaf]) {
        let (Some(left_head), Some(right_head)) = (left.first(), right.first()) else {
            return;
        };
        if !equal(left_head.head, right_head.head) {
            return;
        }
        while let Some((leaf, tail)) = left.split_first() {
            let mut candidates = right;
            while let Some((other, other_tail)) = candidates.split_first() {
                assert!(
                    !equal(leaf.name, other.name),
                    "overlapping canonical instruction selector"
                );
                candidates = other_tail;
            }
            left = tail;
        }
    }

    pub const fn validate(description: &Description) {
        match description {
            Description::Component(dialects) => {
                let mut remaining = *dialects;
                while let Some((dialect, tail)) = remaining.split_first() {
                    let mut others = tail;
                    while let Some((other, other_tail)) = others.split_first() {
                        assert_disjoint_dialects(dialect, other);
                        others = other_tail;
                    }
                    remaining = tail;
                }
            }
            Description::Composite(devices) => {
                let mut remaining = *devices;
                while let Some((device, tail)) = remaining.split_first() {
                    validate(device.child);
                    remaining = tail;
                }
            }
        }
    }
    // Inline const is evaluated for a concrete monomorphization, not by cargo check.
    pub fn description<C: Capabilities>() -> &'static Description {
        const {
            validate(C::DESCRIPTION);
        }
        C::DESCRIPTION
    }
    pub fn flatten(description: &Description) -> Vec<(Vec<&'static str>, Leaf)> {
        fn visit(
            d: &Description,
            path: &mut Vec<&'static str>,
            out: &mut Vec<(Vec<&'static str>, Leaf)>,
        ) {
            match d {
                Description::Component(dialects) => {
                    for dialect in *dialects {
                        for leaf in *dialect {
                            out.push((path.clone(), *leaf));
                        }
                    }
                }
                Description::Composite(devices) => {
                    for device in *devices {
                        path.push(device.name);
                        visit(device.child, path, out);
                        path.pop();
                    }
                }
            }
        }
        let mut out = Vec::new();
        visit(description, &mut Vec::new(), &mut out);
        out
    }

    pub fn prefix<'src>(
        names: &'static [&'static str],
    ) -> chumsky::Boxed<
        'src,
        'src,
        &'src str,
        (),
        chumsky::extra::Err<chumsky::error::Simple<'src, char>>,
    > {
        use chumsky::{Parser, primitive::just};
        let mut names = names.iter();
        let first = names.next().expect("static nonempty field spellings");
        let mut parser = just(*first).then_ignore(just("::")).to(()).boxed();
        for name in names {
            parser = parser
                .or(just(*name).then_ignore(just("::")).to(()))
                .boxed();
        }
        parser
    }
}
