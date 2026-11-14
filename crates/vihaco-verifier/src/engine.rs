// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The verification engine: leaders, reachability, the fixpoint, and replay.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use smallvec::SmallVec;
use vihaco_abi::{Flow, Verify};

use crate::diagnostic::{Diagnostic, DiagnosticKind, classify};
use crate::state::AbstractState;

/// One function to verify: a span of section-local addresses and the state
/// at its entry.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionSpan<S> {
    /// Identifies the function in diagnostics and type maps, typically its
    /// index in the module's function table.
    pub function: u32,
    /// Address of the function's first instruction.
    pub start: u32,
    /// One past the address of the function's last instruction.
    pub end: u32,
    /// The abstract state on entry, for example empty operands and locals
    /// typed from the signature.
    pub entry: S,
}

impl<S> FunctionSpan<S> {
    /// The same span with its entry state transformed, for example to pair a
    /// standard frame with a wrapping context's own entry state.
    pub fn map_entry<T>(self, map: impl FnOnce(S) -> T) -> FunctionSpan<T> {
        FunctionSpan {
            function: self.function,
            start: self.start,
            end: self.end,
            entry: map(self.entry),
        }
    }
}

/// Options for [`verify_functions`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VerifyOptions {
    /// Export each leader's converged state as [`TypeMaps`]. Off by default.
    pub type_maps: bool,
}

impl VerifyOptions {
    /// Enable or disable [`TypeMaps`] output.
    pub fn with_type_maps(mut self, enabled: bool) -> Self {
        self.type_maps = enabled;
        self
    }
}

/// Each reached leader's converged state, by function and address.
///
/// The state at any other address is one block replay away, so these are
/// enough for a debugger or observer to show what each local and operand slot
/// holds at a breakpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeMaps<S> {
    functions: BTreeMap<u32, BTreeMap<u32, S>>,
}

impl<S> Default for TypeMaps<S> {
    fn default() -> Self {
        Self {
            functions: BTreeMap::new(),
        }
    }
}

impl<S> TypeMaps<S> {
    /// The converged state at leader `address` of `function`, if that address
    /// is a reached leader.
    pub fn get(&self, function: u32, address: u32) -> Option<&S> {
        self.functions.get(&function)?.get(&address)
    }

    /// Every reached leader of `function` and its converged state, in address
    /// order.
    pub fn function(&self, function: u32) -> Option<&BTreeMap<u32, S>> {
        self.functions.get(&function)
    }

    /// Every `(function, address, state)` entry, ordered by function then
    /// address.
    pub fn iter(&self) -> impl Iterator<Item = (u32, u32, &S)> {
        self.functions.iter().flat_map(|(&function, leaders)| {
            leaders
                .iter()
                .map(move |(&address, state)| (function, address, state))
        })
    }
}

/// A successful verification: no diagnostic has [`Severity::Error`].
///
/// [`Severity::Error`]: crate::Severity::Error
#[derive(Clone, Debug, PartialEq)]
pub struct Verification<S> {
    /// Warnings, sorted by function then address.
    pub warnings: Vec<Diagnostic>,
    /// Each leader's converged state, when requested through
    /// [`VerifyOptions::type_maps`].
    pub type_maps: Option<TypeMaps<S>>,
}

/// A failed verification: at least one diagnostic has [`Severity::Error`].
///
/// Converts into an [`eyre::Report`]; its `Display` lists every diagnostic.
///
/// [`Severity::Error`]: crate::Severity::Error
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejection {
    /// Every diagnostic, errors and warnings, sorted by function then address.
    pub diagnostics: Vec<Diagnostic>,
}

impl Rejection {
    /// The diagnostics that reject the program.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.is_error())
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.errors().count();
        write!(
            f,
            "verification failed with {count} error{}",
            if count == 1 { "" } else { "s" }
        )?;
        for diagnostic in &self.diagnostics {
            write!(f, "\n  {diagnostic}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Rejection {}

/// Verify each function in `functions` against `code`, the section's
/// instructions indexed by section-local address.
///
/// Verification is intraprocedural: each function is verified on its own
/// from its [`FunctionSpan::entry`] state, through `context`. For each
/// function the engine
///
/// 1. finds **leaders** from [`Verify::flow`] alone: the entry, every jump
///    target inside the function, and every address after a jump;
/// 2. marks **reachable** addresses by walking `flow` alone, reporting
///    [`Unreachable`] runs (a warning), jumps leaving the function
///    ([`InvalidControlFlowTarget`]), and paths falling off its end
///    ([`MissingTerminator`]);
/// 3. runs a worklist **fixpoint** over leaders, transferring each block with
///    [`Verify::verify`] and joining its post-state into its successors'
///    saved states with [`AbstractState::join`], requeueing a leader only when
///    its saved state strictly changes. Errors in this phase are discarded,
///    since blocks see unconverged states;
/// 4. **replays** each reached block once, in address order, from its
///    converged state, reporting the first rule error in the block (which
///    ends that block) and any join mismatch on its outgoing edges (at the
///    join point, once per leader).
///
/// A block whose rule fails contributes nothing to its successors. Every
/// diagnostic of every function is returned, sorted by function then address.
///
/// The fixpoint terminates provided the context's joins have finite height
/// (see [`Lattice`](crate::Lattice)).
///
/// [`Unreachable`]: DiagnosticKind::Unreachable
/// [`InvalidControlFlowTarget`]: DiagnosticKind::InvalidControlFlowTarget
/// [`MissingTerminator`]: DiagnosticKind::MissingTerminator
pub fn verify_functions<I, C>(
    code: &[I],
    functions: &[FunctionSpan<C::Snapshot>],
    context: &mut C,
    options: VerifyOptions,
) -> Result<Verification<C::Snapshot>, Rejection>
where
    I: Verify<C>,
    C: AbstractState + ?Sized,
{
    let mut found = Vec::new();
    let mut type_maps = options.type_maps.then(TypeMaps::default);
    for function in functions {
        let leaders = verify_function(code, function, context, &mut found);
        if let (Some(maps), Some(leaders)) = (type_maps.as_mut(), leaders) {
            maps.functions
                .entry(function.function)
                .or_default()
                .extend(leaders);
        }
    }
    found.sort_by_key(|f| (f.diagnostic.function, f.diagnostic.address, f.order));
    let diagnostics: Vec<Diagnostic> = found.into_iter().map(|f| f.diagnostic).collect();
    if diagnostics.iter().any(Diagnostic::is_error) {
        Err(Rejection { diagnostics })
    } else {
        Ok(Verification {
            warnings: diagnostics,
            type_maps,
        })
    }
}

/// Where a diagnostic sorts among others at the same address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Order {
    /// Joins happen before the leader's instruction runs.
    Join,
    /// Unreachable runs and invalid spans.
    Structure,
    /// The instruction's own rule.
    Rule,
    /// Jump targets, checked after the instruction.
    Target,
    /// Falling off the end, after the last instruction.
    FallOff,
}

struct Found {
    diagnostic: Diagnostic,
    order: Order,
}

/// Verify one function, appending its diagnostics to `found`. Returns each
/// reached leader's converged state (keyed by absolute address), or `None` if
/// the span is invalid.
fn verify_function<I, C>(
    code: &[I],
    function: &FunctionSpan<C::Snapshot>,
    cx: &mut C,
    found: &mut Vec<Found>,
) -> Option<BTreeMap<u32, C::Snapshot>>
where
    I: Verify<C>,
    C: AbstractState + ?Sized,
{
    let FunctionSpan {
        function: id,
        start,
        end,
        ref entry,
    } = *function;
    let mut report = |address: u32, order: Order, kind: DiagnosticKind| {
        found.push(Found {
            diagnostic: Diagnostic::new(id, address, kind),
            order,
        });
    };

    if start > end || end as usize > code.len() {
        let code_len = code.len();
        let kind = DiagnosticKind::InvalidFunctionSpan {
            start,
            end,
            code_len,
        };
        report(start, Order::Structure, kind);
        return None;
    }
    if start == end {
        report(start, Order::FallOff, DiagnosticKind::MissingTerminator);
        return Some(BTreeMap::new());
    }

    let walk = Walk::new(&code[start as usize..end as usize], start);

    // Structural checks, from `flow` alone.
    for (i, flow) in walk.flows.iter().enumerate() {
        let address = start + i as u32;
        if !walk.reached[i] {
            if i == 0 || walk.reached[i - 1] {
                report(address, Order::Structure, DiagnosticKind::Unreachable);
            }
            continue;
        }
        if let Flow::Jump(targets) = flow {
            for &target in targets.iter().filter(|&&t| !walk.contains(t)) {
                let kind = DiagnosticKind::InvalidControlFlowTarget { target };
                report(address, Order::Target, kind);
            }
        }
    }
    let last = walk.len() - 1;
    if walk.reached[last] && walk.flows[last] == Flow::Next {
        report(end - 1, Order::FallOff, DiagnosticKind::MissingTerminator);
    }

    // Phase 1: converge, discarding errors.
    let mut states: Vec<Option<C::Snapshot>> = vec![None; walk.len()];
    states[0] = Some(entry.clone());
    let mut work = BTreeSet::from([0usize]);
    while let Some(leader) = work.pop_first() {
        let saved = states[leader]
            .as_ref()
            .expect("queued leaders have a state");
        let Ok(exit) = walk.run_block(leader, saved, id, cx) else {
            continue;
        };
        let incoming = cx.snapshot();
        for succ in walk.successors(exit) {
            match &mut states[succ] {
                slot @ None => {
                    *slot = Some(incoming.clone());
                    work.insert(succ);
                }
                Some(saved) => {
                    if let Ok(true) = C::join(saved, &incoming) {
                        work.insert(succ);
                    }
                }
            }
        }
    }

    // Phase 2: replay each reached block once, in address order, reporting.
    let mut mismatch_reported = vec![false; walk.len()];
    for leader in 0..walk.len() {
        let Some(saved) = states[leader].as_ref() else {
            continue;
        };
        match walk.run_block(leader, saved, id, cx) {
            Err((at, error)) => report(start + at as u32, Order::Rule, classify(&error)),
            Ok(exit) => {
                let incoming = cx.snapshot();
                for succ in walk.successors(exit) {
                    let Some(saved) = &states[succ] else { continue };
                    let mut probe = saved.clone();
                    if let Err(error) = C::join(&mut probe, &incoming)
                        && !std::mem::replace(&mut mismatch_reported[succ], true)
                    {
                        report(start + succ as u32, Order::Join, error.into_kind());
                    }
                }
            }
        }
    }

    Some(
        states
            .into_iter()
            .enumerate()
            .filter_map(|(i, state)| Some((start + i as u32, state?)))
            .collect(),
    )
}

/// One function's body with its control-flow facts, which depend only on
/// [`Verify::flow`].
struct Walk<'a, I> {
    body: &'a [I],
    start: u32,
    flows: Vec<Flow>,
    leader: Vec<bool>,
    reached: Vec<bool>,
}

impl<'a, I> Walk<'a, I> {
    fn new<C: ?Sized>(body: &'a [I], start: u32) -> Self
    where
        I: Verify<C>,
    {
        let flows: Vec<Flow> = body.iter().map(<I as Verify<C>>::flow).collect();
        let mut walk = Walk {
            body,
            start,
            leader: vec![false; flows.len()],
            reached: vec![false; flows.len()],
            flows,
        };
        walk.leader[0] = true;
        for i in 0..walk.len() {
            if let Flow::Jump(targets) = &walk.flows[i] {
                for &target in targets {
                    if let Some(t) = walk.offset(target) {
                        walk.leader[t] = true;
                    }
                }
                if i + 1 < walk.len() {
                    walk.leader[i + 1] = true;
                }
            }
        }
        let mut pending = vec![0usize];
        while let Some(i) = pending.pop() {
            if !std::mem::replace(&mut walk.reached[i], true) {
                pending.extend(walk.successors(i));
            }
        }
        walk
    }

    fn len(&self) -> usize {
        self.body.len()
    }

    fn contains(&self, address: u32) -> bool {
        self.offset(address).is_some()
    }

    /// The body offset of a section-local address inside this function.
    fn offset(&self, address: u32) -> Option<usize> {
        let offset = address.checked_sub(self.start)? as usize;
        (offset < self.len()).then_some(offset)
    }

    /// Successors of the instruction at `i` inside this function. Targets
    /// outside it are not followed; the target check reports them.
    fn successors(&self, i: usize) -> SmallVec<[usize; 2]> {
        match &self.flows[i] {
            Flow::Next if i + 1 < self.len() => SmallVec::from_buf_and_len([i + 1, 0], 1),
            Flow::Jump(targets) => {
                let mut succs: SmallVec<[usize; 2]> = SmallVec::new();
                for t in targets.iter().filter_map(|&t| self.offset(t)) {
                    if !succs.contains(&t) {
                        succs.push(t);
                    }
                }
                succs
            }
            _ => SmallVec::new(),
        }
    }

    /// Transfer the block starting at `leader` from `state`, leaving the
    /// post-state in `cx`. Returns the offset of the block's last instruction,
    /// or the offset and error of the first rule that fails.
    fn run_block<C>(
        &self,
        leader: usize,
        state: &C::Snapshot,
        function: u32,
        cx: &mut C,
    ) -> Result<usize, (usize, eyre::Report)>
    where
        I: Verify<C>,
        C: AbstractState + ?Sized,
    {
        cx.restore(state);
        let mut i = leader;
        loop {
            cx.set_address(function, self.start + i as u32);
            self.body[i].verify(cx).map_err(|error| (i, error))?;
            match self.flows[i] {
                Flow::Next if i + 1 < self.len() && !self.leader[i + 1] => i += 1,
                _ => return Ok(i),
            }
        }
    }
}
