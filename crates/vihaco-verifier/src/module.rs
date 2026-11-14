// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The standard verifier context for modules: [`ModuleVerifier`], and
//! [`WithState`] for machines that add component state.

use std::collections::HashMap;

use vihaco_abi::{Type, Value, Verify};
use vihaco_module::module::{FunctionInfo, LocalModule, MAX_LOCALS, Signature, StructInfo};

use crate::capability::{Constants, Locals, Signatures, StructTypes};
use crate::diagnostic::{Diagnostic, DiagnosticKind, VerifyError};
use crate::engine::{FunctionSpan, Rejection, Verification, VerifyOptions, verify_functions};
use crate::slot::TypeSlot;
use crate::stack::{Frame, FrameState, OperandStack};
use crate::state::AbstractState;

/// The standard verifier context for a module using vihaco's standard model:
/// [`Type`] for types, [`Value`] for constants, and [`TypeSlot`] slots.
///
/// It reads the module's metadata tables and implements every standard
/// capability: [`OperandStack`], [`Locals`], [`Signatures<Type>`],
/// [`StructTypes<Type>`] and [`Constants<Value, Type>`], plus
/// [`AbstractState`] over a standard [`Frame`]. Each function is verified
/// from its entry state: an empty operand stack and locals typed from the
/// function's declared `locals` (parameters first).
///
/// Use [`verify_module`] (or [`ModuleVerifier::verify`]) to verify every
/// function of a module. A machine whose components need more state wraps
/// this context: [`WithState`] adds a [`ComponentState`] joined alongside the
/// frame, or a hand-written context can delegate to [`frame_state_mut`] and
/// build its entry states with [`function_spans`] and
/// [`FunctionSpan::map_entry`].
///
/// [`frame_state_mut`]: ModuleVerifier::frame_state_mut
/// [`function_spans`]: ModuleVerifier::function_spans
#[derive(Clone, Debug)]
pub struct ModuleVerifier<'m> {
    functions: &'m [FunctionInfo<Type>],
    structs: &'m [StructInfo<Type>],
    constants: &'m [Value],
    strings: &'m [String],
    by_address: HashMap<u32, u32>,
    frame: FrameState<TypeSlot>,
}

impl<'m> ModuleVerifier<'m> {
    /// A verifier for `module`'s metadata tables. The code is supplied
    /// separately to [`verify`](ModuleVerifier::verify).
    pub fn new<I, Info>(module: &'m LocalModule<I, Value, Type, Info>) -> Self {
        Self::from_parts(
            &module.functions,
            &module.structs,
            &module.constants,
            &module.strings,
        )
    }

    /// A verifier for metadata tables held outside a [`LocalModule`].
    pub fn from_parts(
        functions: &'m [FunctionInfo<Type>],
        structs: &'m [StructInfo<Type>],
        constants: &'m [Value],
        strings: &'m [String],
    ) -> Self {
        let mut by_address = HashMap::with_capacity(functions.len());
        for (index, function) in functions.iter().enumerate() {
            // The first function starting at an address wins, as at runtime.
            by_address
                .entry(function.start_address)
                .or_insert(index as u32);
        }
        Self {
            functions,
            structs,
            constants,
            strings,
            by_address,
            frame: FrameState::new(Frame::new(Vec::new())),
        }
    }

    /// The function table.
    pub fn functions(&self) -> &'m [FunctionInfo<Type>] {
        self.functions
    }

    /// The struct table.
    pub fn structs(&self) -> &'m [StructInfo<Type>] {
        self.structs
    }

    /// The constant table.
    pub fn constants(&self) -> &'m [Value] {
        self.constants
    }

    /// The string table.
    pub fn strings(&self) -> &'m [String] {
        self.strings
    }

    /// The current abstract frame and position.
    pub fn frame_state(&self) -> &FrameState<TypeSlot> {
        &self.frame
    }

    /// The current abstract frame and position, mutably, for wrapping
    /// contexts that implement [`AbstractState`] themselves.
    pub fn frame_state_mut(&mut self) -> &mut FrameState<TypeSlot> {
        &mut self.frame
    }

    /// The entry frame of `function`: no operands, and one local per declared
    /// local, typed as declared. `None` for an unknown function.
    pub fn entry_frame(&self, function: u32) -> Option<Frame<TypeSlot>> {
        let info = self.functions.get(function as usize)?;
        Some(Frame::new(
            info.locals.iter().cloned().map(TypeSlot::Known).collect(),
        ))
    }

    /// One [`FunctionSpan`] per function, in table order, with its entry
    /// frame. Functions with invalid frame metadata (see
    /// [`check_frames`](ModuleVerifier::check_frames)) are skipped.
    pub fn function_spans(&self) -> Vec<FunctionSpan<Frame<TypeSlot>>> {
        (0..self.functions.len() as u32)
            .filter(|&function| self.frame_error(function).is_none())
            .filter_map(|function| {
                let info = &self.functions[function as usize];
                Some(FunctionSpan {
                    function,
                    start: info.start_address,
                    end: info.end_address,
                    entry: self.entry_frame(function)?,
                })
            })
            .collect()
    }

    /// Check each function's frame metadata: at most [`MAX_LOCALS`] locals,
    /// at least as many locals as parameters, and leading locals equal to the
    /// parameter types. [`ProgramMetadata::validate`] guarantees all three;
    /// this repeats them for modules built directly. Each failure is a
    /// [`DiagnosticKind::Rule`] error at the function's start address.
    ///
    /// [`ProgramMetadata::validate`]: vihaco_module::module::ProgramMetadata::validate
    pub fn check_frames(&self) -> Vec<Diagnostic> {
        (0..self.functions.len() as u32)
            .filter_map(|function| {
                let kind = self.frame_error(function)?;
                let start = self.functions[function as usize].start_address;
                Some(Diagnostic::new(function, start, kind))
            })
            .collect()
    }

    fn frame_error(&self, function: u32) -> Option<DiagnosticKind> {
        let info = &self.functions[function as usize];
        let (locals, params) = (&info.locals, &info.signature.params);
        let message = if locals.len() > MAX_LOCALS {
            format!(
                "function declares {} locals, more than the maximum of {MAX_LOCALS}",
                locals.len()
            )
        } else if locals.len() < params.len() {
            format!(
                "function declares {} locals but {} parameters",
                locals.len(),
                params.len()
            )
        } else {
            let (index, (local, param)) = locals
                .iter()
                .zip(params)
                .enumerate()
                .find(|(_, (local, param))| **local != param.ty)?;
            format!(
                "local {index} is declared {local}, but parameter {index} is {}",
                param.ty
            )
        };
        Some(DiagnosticKind::Rule(message))
    }

    /// Verify every function of the module against `code`, the module's
    /// instructions. Reports frame-metadata errors from
    /// [`check_frames`](ModuleVerifier::check_frames) alongside the engine's
    /// diagnostics (see [`verify_functions`]).
    pub fn verify<I>(
        &mut self,
        code: &[I],
        options: VerifyOptions,
    ) -> Result<Verification<Frame<TypeSlot>>, Rejection>
    where
        I: Verify<Self>,
    {
        let spans = self.function_spans();
        let frames = self.check_frames();
        merge(frames, verify_functions(code, &spans, self, options))
    }

    /// Wrap this verifier with component state, starting every function
    /// from `entry`.
    pub fn with_state<S: ComponentState>(self, entry: S) -> WithState<'m, S> {
        WithState::new(self, entry)
    }

    fn function_info(&self, function: u32) -> Result<&'m FunctionInfo<Type>, VerifyError> {
        self.functions.get(function as usize).ok_or_else(|| {
            VerifyError::rule(format!(
                "function {function} is outside the function table of length {}",
                self.functions.len()
            ))
        })
    }

    fn struct_info(&self, struct_id: u32) -> Result<&'m StructInfo<Type>, VerifyError> {
        self.structs.get(struct_id as usize).ok_or_else(|| {
            VerifyError::rule(format!(
                "struct {struct_id} is outside the struct table of length {}",
                self.structs.len()
            ))
        })
    }
}

/// Verify every function of `module` with a fresh [`ModuleVerifier`].
///
/// Shorthand for `ModuleVerifier::new(module).verify(&module.code, options)`.
pub fn verify_module<'m, I, Info>(
    module: &'m LocalModule<I, Value, Type, Info>,
    options: VerifyOptions,
) -> Result<Verification<Frame<TypeSlot>>, Rejection>
where
    I: Verify<ModuleVerifier<'m>>,
{
    ModuleVerifier::new(module).verify(&module.code, options)
}

/// Add pre-engine diagnostics to an engine result, keeping the order by
/// function then address (pre-engine diagnostics first at equal positions).
fn merge<S>(
    mut diagnostics: Vec<Diagnostic>,
    result: Result<Verification<S>, Rejection>,
) -> Result<Verification<S>, Rejection> {
    if diagnostics.is_empty() {
        return result;
    }
    let type_maps = match result {
        Ok(verification) => {
            diagnostics.extend(verification.warnings);
            verification.type_maps
        }
        Err(rejection) => {
            diagnostics.extend(rejection.diagnostics);
            None
        }
    };
    diagnostics.sort_by_key(|d| (d.function, d.address));
    if diagnostics.iter().any(Diagnostic::is_error) {
        Err(Rejection { diagnostics })
    } else {
        Ok(Verification {
            warnings: diagnostics,
            type_maps,
        })
    }
}

impl OperandStack for ModuleVerifier<'_> {
    type Slot = TypeSlot;

    fn pop(&mut self) -> Result<TypeSlot, VerifyError> {
        self.frame.pop()
    }

    fn pop_n(&mut self, n: usize) -> Result<Vec<TypeSlot>, VerifyError> {
        self.frame.pop_n(n)
    }

    fn push(&mut self, slot: TypeSlot) {
        self.frame.push(slot);
    }

    fn peek(&self) -> Result<&TypeSlot, VerifyError> {
        self.frame.peek()
    }

    fn depth(&self) -> usize {
        self.frame.depth()
    }
}

impl Locals for ModuleVerifier<'_> {
    fn local_count(&self) -> u32 {
        self.frame.local_count()
    }

    fn local(&self, index: u32) -> Result<&TypeSlot, VerifyError> {
        self.frame.local(index)
    }

    fn set_local(&mut self, index: u32, slot: TypeSlot) -> Result<(), VerifyError> {
        self.frame.set_local(index, slot)
    }
}

impl Signatures<Type> for ModuleVerifier<'_> {
    fn function_count(&self) -> usize {
        self.functions.len()
    }

    fn signature(&self, function: u32) -> Result<&Signature<Type>, VerifyError> {
        Ok(&self.function_info(function)?.signature)
    }

    fn function_at(&self, address: u32) -> Result<u32, VerifyError> {
        self.by_address
            .get(&address)
            .copied()
            .ok_or_else(|| VerifyError::rule(format!("no function starts at address {address}")))
    }

    fn current_function(&self) -> u32 {
        self.frame.function()
    }
}

impl StructTypes<Type> for ModuleVerifier<'_> {
    fn struct_count(&self) -> usize {
        self.structs.len()
    }

    fn field_count(&self, struct_id: u32) -> Result<usize, VerifyError> {
        Ok(self.struct_info(struct_id)?.fields.len())
    }

    fn field_type(&self, struct_id: u32, slot: u32) -> Result<&Type, VerifyError> {
        let info = self.struct_info(struct_id)?;
        info.fields
            .get(slot as usize)
            .map(|field| &field.ty)
            .ok_or_else(|| {
                VerifyError::rule(format!(
                    "field slot {slot} is outside struct {struct_id}, which has {} fields",
                    info.fields.len()
                ))
            })
    }
}

impl Constants<Value, Type> for ModuleVerifier<'_> {
    fn constant_count(&self) -> usize {
        self.constants.len()
    }

    fn constant(&self, index: u32) -> Result<&Value, VerifyError> {
        self.constants.get(index as usize).ok_or_else(|| {
            VerifyError::rule(format!(
                "constant {index} is outside the constant table of length {}",
                self.constants.len()
            ))
        })
    }

    fn constant_type(&self, index: u32) -> Result<Type, VerifyError> {
        Ok(self.constant(index)?.type_of())
    }

    fn string_count(&self) -> usize {
        self.strings.len()
    }
}

impl AbstractState for ModuleVerifier<'_> {
    type Snapshot = Frame<TypeSlot>;

    fn snapshot(&self) -> Frame<TypeSlot> {
        self.frame.snapshot()
    }

    fn restore(&mut self, snapshot: &Frame<TypeSlot>) {
        self.frame.restore(snapshot);
    }

    fn join(saved: &mut Frame<TypeSlot>, incoming: &Frame<TypeSlot>) -> Result<bool, VerifyError> {
        saved.join(incoming)
    }

    /// Records the current function, which [`Signatures::current_function`]
    /// reports.
    fn set_address(&mut self, function: u32, address: u32) {
        self.frame.set_address(function, address);
    }
}

/// Abstract state a component adds to a verifier context, joined at every
/// leader alongside the frame.
///
/// Implement this for a component's state (for example a set of open
/// channels, joined by intersection) and verify with
/// [`WithState<'_, YourState>`](WithState). Several components' states
/// combine as a tuple.
pub trait ComponentState: Clone {
    /// Join `incoming` into `self`, the state saved at a join point.
    ///
    /// Returns whether `self` strictly changed. On error `self` must be left
    /// unchanged. The join must have finite height so the fixpoint
    /// terminates: widen unbounded values (such as elapsed time) after a
    /// fixed number of joins.
    fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError>;
}

impl ComponentState for () {
    fn join(&mut self, _: &()) -> Result<bool, VerifyError> {
        Ok(false)
    }
}

macro_rules! tuple_component_state {
    ($($name:ident $index:tt),+) => {
        /// Joins each element; on error no element changes.
        impl<$($name: ComponentState),+> ComponentState for ($($name,)+) {
            fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError> {
                let mut joined = self.clone();
                let mut changed = false;
                $(changed |= joined.$index.join(&incoming.$index)?;)+
                *self = joined;
                Ok(changed)
            }
        }
    };
}

tuple_component_state!(A 0);
tuple_component_state!(A 0, B 1);
tuple_component_state!(A 0, B 1, C 2);
tuple_component_state!(A 0, B 1, C 2, D 3);

/// A [`ModuleVerifier`] extended with component state `S`: the context for a
/// machine whose components need more than the standard capabilities.
///
/// It delegates every standard capability to the inner verifier, snapshots
/// and joins `S` alongside the frame (snapshots are `(Frame<TypeSlot>, S)`),
/// and starts every function from the entry state given at construction.
/// A component crate defines its capability trait and implements it for
/// `WithState<'_, TheirState>` (allowed because the trait is theirs), using
/// [`state`](WithState::state) and [`state_mut`](WithState::state_mut):
///
/// ```
/// use std::collections::BTreeSet;
/// use vihaco_verifier::{ComponentState, VerifyError, WithState};
///
/// /// The sequencer's capability: channels must be opened before use.
/// pub trait Channels {
///     fn open(&mut self, channel: u32);
///     fn require_open(&self, channel: u32) -> Result<(), VerifyError>;
/// }
///
/// #[derive(Clone, Default)]
/// pub struct Open(BTreeSet<u32>);
///
/// impl ComponentState for Open {
///     /// A channel is open after a join only if it is open on every path.
///     fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError> {
///         let before = self.0.len();
///         self.0.retain(|c| incoming.0.contains(c));
///         Ok(self.0.len() != before)
///     }
/// }
///
/// impl Channels for WithState<'_, Open> {
///     fn open(&mut self, channel: u32) {
///         self.state_mut().0.insert(channel);
///     }
///     fn require_open(&self, channel: u32) -> Result<(), VerifyError> {
///         if self.state().0.contains(&channel) {
///             Ok(())
///         } else {
///             Err(VerifyError::rule(format!("channel {channel} is not open")))
///         }
///     }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct WithState<'m, S> {
    verifier: ModuleVerifier<'m>,
    entry: S,
    state: S,
}

impl<'m, S: ComponentState> WithState<'m, S> {
    /// Wrap `verifier`, starting every function with component state `entry`.
    pub fn new(verifier: ModuleVerifier<'m>, entry: S) -> Self {
        Self {
            verifier,
            state: entry.clone(),
            entry,
        }
    }

    /// The inner standard verifier.
    pub fn verifier(&self) -> &ModuleVerifier<'m> {
        &self.verifier
    }

    /// The inner standard verifier, mutably.
    pub fn verifier_mut(&mut self) -> &mut ModuleVerifier<'m> {
        &mut self.verifier
    }

    /// Unwrap the inner standard verifier.
    pub fn into_verifier(self) -> ModuleVerifier<'m> {
        self.verifier
    }

    /// The current component state.
    pub fn state(&self) -> &S {
        &self.state
    }

    /// The current component state, mutably.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    /// One [`FunctionSpan`] per function with valid frame metadata, entering
    /// with the standard frame and the entry component state.
    pub fn function_spans(&self) -> Vec<FunctionSpan<(Frame<TypeSlot>, S)>> {
        self.verifier
            .function_spans()
            .into_iter()
            .map(|span| span.map_entry(|frame| (frame, self.entry.clone())))
            .collect()
    }

    /// Verify every function of the module against `code`, as
    /// [`ModuleVerifier::verify`] does, with component state.
    pub fn verify<I>(
        &mut self,
        code: &[I],
        options: VerifyOptions,
    ) -> Result<Verification<(Frame<TypeSlot>, S)>, Rejection>
    where
        I: Verify<Self>,
    {
        let spans = self.function_spans();
        let frames = self.verifier.check_frames();
        merge(frames, verify_functions(code, &spans, self, options))
    }
}

impl<S> OperandStack for WithState<'_, S> {
    type Slot = TypeSlot;

    fn pop(&mut self) -> Result<TypeSlot, VerifyError> {
        self.verifier.pop()
    }

    fn pop_n(&mut self, n: usize) -> Result<Vec<TypeSlot>, VerifyError> {
        self.verifier.pop_n(n)
    }

    fn push(&mut self, slot: TypeSlot) {
        self.verifier.push(slot);
    }

    fn peek(&self) -> Result<&TypeSlot, VerifyError> {
        self.verifier.peek()
    }

    fn depth(&self) -> usize {
        self.verifier.depth()
    }
}

impl<S> Locals for WithState<'_, S> {
    fn local_count(&self) -> u32 {
        self.verifier.local_count()
    }

    fn local(&self, index: u32) -> Result<&TypeSlot, VerifyError> {
        self.verifier.local(index)
    }

    fn set_local(&mut self, index: u32, slot: TypeSlot) -> Result<(), VerifyError> {
        self.verifier.set_local(index, slot)
    }
}

impl<S> Signatures<Type> for WithState<'_, S> {
    fn function_count(&self) -> usize {
        self.verifier.function_count()
    }

    fn signature(&self, function: u32) -> Result<&Signature<Type>, VerifyError> {
        self.verifier.signature(function)
    }

    fn function_at(&self, address: u32) -> Result<u32, VerifyError> {
        self.verifier.function_at(address)
    }

    fn current_function(&self) -> u32 {
        self.verifier.current_function()
    }
}

impl<S> StructTypes<Type> for WithState<'_, S> {
    fn struct_count(&self) -> usize {
        self.verifier.struct_count()
    }

    fn field_count(&self, struct_id: u32) -> Result<usize, VerifyError> {
        self.verifier.field_count(struct_id)
    }

    fn field_type(&self, struct_id: u32, slot: u32) -> Result<&Type, VerifyError> {
        self.verifier.field_type(struct_id, slot)
    }
}

impl<S> Constants<Value, Type> for WithState<'_, S> {
    fn constant_count(&self) -> usize {
        self.verifier.constant_count()
    }

    fn constant(&self, index: u32) -> Result<&Value, VerifyError> {
        self.verifier.constant(index)
    }

    fn constant_type(&self, index: u32) -> Result<Type, VerifyError> {
        self.verifier.constant_type(index)
    }

    fn string_count(&self) -> usize {
        self.verifier.string_count()
    }
}

impl<S: ComponentState> AbstractState for WithState<'_, S> {
    type Snapshot = (Frame<TypeSlot>, S);

    fn snapshot(&self) -> Self::Snapshot {
        (self.verifier.snapshot(), self.state.clone())
    }

    fn restore(&mut self, (frame, state): &Self::Snapshot) {
        self.verifier.restore(frame);
        self.state.clone_from(state);
    }

    fn join(
        (frame, state): &mut Self::Snapshot,
        (in_frame, in_state): &Self::Snapshot,
    ) -> Result<bool, VerifyError> {
        // Join the component state on a copy first, so an error in either
        // join leaves the saved snapshot unchanged.
        let mut joined = state.clone();
        let state_changed = joined.join(in_state)?;
        let frame_changed = frame.join(in_frame)?;
        *state = joined;
        Ok(frame_changed || state_changed)
    }

    fn set_address(&mut self, function: u32, address: u32) {
        self.verifier.set_address(function, address);
    }
}
