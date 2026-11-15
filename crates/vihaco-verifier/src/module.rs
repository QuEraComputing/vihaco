// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! The standard verifier context for modules: [`ModuleVerifier`], and
//! [`WithState`] for machines that add component state.

use std::collections::HashMap;

use vihaco_abi::{Type, Value, Verify};
use vihaco_module::module::{FunctionInfo, LocalModule, NamedTypeInfo, Signature, StructInfo};

use crate::capability::{Constants, Locals, NamedTypes, Signatures, StructTypes};
use crate::diagnostic::{Diagnostic, DiagnosticKind, Table, VerifyError};
use crate::engine::{FunctionSpan, Rejection, Verification, VerifyOptions, verify_functions};
use crate::slot::TypeSlot;
use crate::stack::{Frame, FrameState, OperandStack};
use crate::state::{AbstractState, JoinPoint};

/// The standard verifier context for a module using vihaco's standard model:
/// [`Type`] for types, [`Value`] for constants, and [`TypeSlot`] slots.
///
/// It reads the module's metadata tables and implements every standard
/// capability: [`OperandStack`], [`Locals`], [`Signatures<Type>`],
/// [`StructTypes<Type>`], [`NamedTypes<Type>`] and [`Constants<Value, Type>`], plus
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
    types: &'m [NamedTypeInfo<Type>],
    constants: &'m [Value],
    strings: &'m [String],
    by_address: HashMap<u32, u32>,
    type_names: HashMap<&'m str, u32>,
    frame: FrameState<TypeSlot>,
}

impl<'m> ModuleVerifier<'m> {
    /// A verifier for `module`'s metadata tables. The code is supplied
    /// separately to [`verify`](ModuleVerifier::verify).
    pub fn new<I, Info>(module: &'m LocalModule<I, Value, Type, Info>) -> Self {
        Self::from_parts(
            &module.functions,
            &module.structs,
            &module.types,
            &module.constants,
            &module.strings,
        )
    }

    /// A verifier for metadata tables held outside a [`LocalModule`].
    pub fn from_parts(
        functions: &'m [FunctionInfo<Type>],
        structs: &'m [StructInfo<Type>],
        types: &'m [NamedTypeInfo<Type>],
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
        let mut type_names = HashMap::with_capacity(types.len());
        for (index, info) in types.iter().enumerate() {
            // Names are unique in validated metadata; keep the first otherwise.
            if let Some(name) = strings.get(info.name as usize) {
                type_names.entry(name.as_str()).or_insert(index as u32);
            }
        }
        Self {
            functions,
            structs,
            types,
            constants,
            strings,
            by_address,
            type_names,
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

    /// The named-type table.
    pub fn types(&self) -> &'m [NamedTypeInfo<Type>] {
        self.types
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
    /// parameter types (see [`FunctionInfo::check_frame`]).
    /// [`ProgramMetadata::validate`] guarantees all three; this repeats them
    /// for modules built directly. Each failure is a
    /// [`DiagnosticKind::InvalidFrame`] error at the function's start address.
    ///
    /// [`ProgramMetadata::validate`]: vihaco_module::module::ProgramMetadata::validate
    /// [`MAX_LOCALS`]: vihaco_module::module::MAX_LOCALS
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
        info.check_frame().err().map(DiagnosticKind::InvalidFrame)
    }

    /// Verify every function of the module against `code`, the module's
    /// instructions. Reports frame-metadata errors from
    /// [`check_frames`](ModuleVerifier::check_frames) alongside the engine's
    /// diagnostics (see [`verify_functions`]).
    pub fn verify<I>(
        &mut self,
        code: &[I],
        options: VerifyOptions,
    ) -> Result<Verification<Frame<TypeSlot>>, Rejection<Frame<TypeSlot>>>
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
            VerifyError::unknown_entry(Table::Functions, function, self.functions.len())
        })
    }

    fn struct_info(&self, struct_id: u32) -> Result<&'m StructInfo<Type>, VerifyError> {
        self.structs.get(struct_id as usize).ok_or_else(|| {
            VerifyError::unknown_entry(Table::Structs, struct_id, self.structs.len())
        })
    }
}

/// Verify every function of `module` with a fresh [`ModuleVerifier`].
///
/// Shorthand for `ModuleVerifier::new(module).verify(&module.code, options)`.
pub fn verify_module<'m, I, Info>(
    module: &'m LocalModule<I, Value, Type, Info>,
    options: VerifyOptions,
) -> Result<Verification<Frame<TypeSlot>>, Rejection<Frame<TypeSlot>>>
where
    I: Verify<ModuleVerifier<'m>>,
{
    ModuleVerifier::new(module).verify(&module.code, options)
}

/// Add pre-engine diagnostics to an engine result, keeping the order by
/// function then address (pre-engine diagnostics first at equal positions).
fn merge<S>(
    mut diagnostics: Vec<Diagnostic>,
    result: Result<Verification<S>, Rejection<S>>,
) -> Result<Verification<S>, Rejection<S>> {
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
            rejection.type_maps
        }
    };
    diagnostics.sort_by_key(|d| (d.function, d.address));
    if diagnostics.iter().any(Diagnostic::is_error) {
        Err(Rejection {
            diagnostics,
            type_maps,
        })
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
            .ok_or_else(|| VerifyError::new(DiagnosticKind::NoFunctionAt { address }))
    }

    fn current_function(&self) -> u32 {
        self.frame.function()
    }

    /// The frame is the only state, and the call's operand effect already
    /// covers it: the callee cannot touch the caller's locals or the operands
    /// below its arguments.
    fn after_call(&mut self, _callee: Option<u32>) {}
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
                VerifyError::new(DiagnosticKind::UnknownField {
                    struct_id,
                    field: slot,
                    field_count: info.fields.len(),
                })
            })
    }
}

impl NamedTypes<Type> for ModuleVerifier<'_> {
    fn named_type_count(&self) -> usize {
        self.types.len()
    }

    fn named_type_index(&self, name: &str) -> Result<u32, VerifyError> {
        self.type_names.get(name).copied().ok_or_else(|| {
            VerifyError::new(DiagnosticKind::UndefinedNamedType {
                name: name.to_owned(),
            })
        })
    }

    fn named_type_repr(&self, index: u32) -> Result<&Type, VerifyError> {
        self.types
            .get(index as usize)
            .map(|info| &info.repr)
            .ok_or_else(|| VerifyError::unknown_entry(Table::NamedTypes, index, self.types.len()))
    }
}

impl Constants<Value, Type> for ModuleVerifier<'_> {
    fn constant_count(&self) -> usize {
        self.constants.len()
    }

    fn constant(&self, index: u32) -> Result<&Value, VerifyError> {
        self.constants.get(index as usize).ok_or_else(|| {
            VerifyError::unknown_entry(Table::Constants, index, self.constants.len())
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

    fn join(
        saved: &mut Frame<TypeSlot>,
        incoming: &Frame<TypeSlot>,
        at: &JoinPoint,
    ) -> Result<bool, VerifyError> {
        saved.join(incoming, at)
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
/// combine into one struct with [`component_state!`](crate::component_state),
/// which joins each field.
///
/// # Calls
///
/// Every function is verified separately, starting from the entry state given
/// to [`WithState::new`]. Because any function may be called from anywhere,
/// the entry state must assume nothing a caller might not have established:
/// typically the empty state (no channels open, nothing elapsed). After a
/// call, the caller's state becomes [`after_call`](Self::after_call), which
/// every component state defines, because what is sound depends on the
/// state; per-callee summaries are a possible future extension.
pub trait ComponentState: Clone {
    /// Join `incoming` into `self`, the state saved at a join point.
    ///
    /// Returns whether `self` strictly changed. On error `self` must be left
    /// unchanged. The join must have finite height so the fixpoint
    /// terminates: widen unbounded values (such as elapsed time) after a
    /// fixed number of joins.
    fn join(&mut self, incoming: &Self) -> Result<bool, VerifyError>;

    /// The caller's state after a call returns. `callee` is the called
    /// function for a direct call and `None` for an indirect one; `entry` is
    /// the state every function starts from.
    ///
    /// The result must assume no more than any callee could leave behind.
    /// Resetting to `entry` (`self.clone_from(entry)`) does that only when
    /// `entry` also describes every state a callee may return in. It does
    /// for a set of open channels whose entry is "none open": whatever the
    /// callee opened or closed, assuming none open is safe. It does not for
    /// an accumulating quantity such as elapsed time, whose entry is "0
    /// elapsed" while a callee may spend any amount; resetting it would
    /// accept `wait 2; call @helper; deadline 10` even if the helper waits
    /// 50, so such a state must become "unknown" instead.
    ///
    /// There is no default, so a state cannot get this wrong by omission
    /// (like [`Signatures::after_call`]). It is applied by the call helpers
    /// ([`call_effect`](crate::call_effect),
    /// [`indirect_call_effect`](crate::indirect_call_effect)); a custom
    /// call-like rule must use them or call [`Signatures::after_call`]
    /// itself.
    fn after_call(&mut self, callee: Option<u32>, entry: &Self);
}

impl ComponentState for () {
    fn join(&mut self, _: &()) -> Result<bool, VerifyError> {
        Ok(false)
    }

    fn after_call(&mut self, _: Option<u32>, _: &()) {}
}

/// Where one component's state lives inside a context's combined state.
///
/// A component crate implements its capability for every context whose state
/// contains its own, rather than for one concrete state:
///
/// ```ignore
/// impl<S: ComponentStateOf<Open>> Channels for WithState<'_, S> { ... }
/// ```
///
/// so the capability works in a machine that combines it with other
/// components' states. Every state contains itself, and
/// [`component_state!`](crate::component_state) implements this for each
/// field of a combined state, so no crate writes delegation by hand (and the
/// orphan rule never gets in the way: the machine crate owns its combined
/// state).
pub trait ComponentStateOf<T> {
    /// The component's state.
    fn component(&self) -> &T;

    /// The component's state, mutably.
    fn component_mut(&mut self) -> &mut T;
}

impl<T> ComponentStateOf<T> for T {
    fn component(&self) -> &T {
        self
    }

    fn component_mut(&mut self) -> &mut T {
        self
    }
}

/// Declare a machine's combined component state: a struct with one field per
/// component's [`ComponentState`].
///
/// The struct joins field by field (atomically: on error no field changes),
/// applies each field's [`ComponentState::after_call`] after a call, and
/// implements [`ComponentStateOf`] for each field's type, so every
/// component's capability, written against `ComponentStateOf<TheirState>`,
/// works in the machine's [`WithState`] context. Derive `Clone` (required)
/// and usually `Default` (a convenient entry state). Field types must be
/// distinct.
///
/// ```
/// use vihaco_verifier::{ComponentState, VerifyError, component_state};
/// # #[derive(Clone, Debug, Default)]
/// # pub struct Open;
/// # impl ComponentState for Open {
/// #     fn join(&mut self, _: &Self) -> Result<bool, VerifyError> { Ok(false) }
/// #     fn after_call(&mut self, _: Option<u32>, entry: &Self) { self.clone_from(entry) }
/// # }
/// # #[derive(Clone, Debug, Default)]
/// # pub struct Elapsed;
/// # impl ComponentState for Elapsed {
/// #     fn join(&mut self, _: &Self) -> Result<bool, VerifyError> { Ok(false) }
/// #     fn after_call(&mut self, _: Option<u32>, entry: &Self) { self.clone_from(entry) }
/// # }
///
/// // `Open` from a sequencer crate, `Elapsed` from a timing crate.
/// component_state! {
///     #[derive(Clone, Debug, Default)]
///     pub struct MachineState {
///         pub open: Open,
///         pub elapsed: Elapsed,
///     }
/// }
/// ```
#[macro_export]
macro_rules! component_state {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($(#[$field_meta:meta])* $field_vis:vis $field:ident: $ty:ty),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $($(#[$field_meta])* $field_vis $field: $ty),*
        }

        impl $crate::ComponentState for $name {
            fn join(&mut self, incoming: &Self) -> ::core::result::Result<bool, $crate::VerifyError> {
                // Join a copy, so an error in any field leaves `self` whole.
                let mut joined = ::core::clone::Clone::clone(self);
                let mut changed = false;
                $(changed |= $crate::ComponentState::join(&mut joined.$field, &incoming.$field)?;)*
                *self = joined;
                Ok(changed)
            }

            /// Applies each field's own `after_call`.
            fn after_call(&mut self, callee: ::core::option::Option<u32>, entry: &Self) {
                $($crate::ComponentState::after_call(&mut self.$field, callee, &entry.$field);)*
            }
        }

        $(
            impl $crate::ComponentStateOf<$ty> for $name {
                fn component(&self) -> &$ty {
                    &self.$field
                }

                fn component_mut(&mut self) -> &mut $ty {
                    &mut self.$field
                }
            }
        )*
    };
}

/// A [`ModuleVerifier`] extended with component state `S`: the context for a
/// machine whose components need more than the standard capabilities.
///
/// It delegates every standard capability to the inner verifier, snapshots
/// and joins `S` alongside the frame (snapshots are `(Frame<TypeSlot>, S)`),
/// and starts every function from the entry state given at construction.
/// A component crate defines its capability trait and implements it for
/// `WithState<'_, S>` wherever `S` contains its state ([`ComponentStateOf`]),
/// reaching that state with [`component`](WithState::component) and
/// [`component_mut`](WithState::component_mut):
///
/// ```
/// use std::collections::BTreeSet;
/// use vihaco_verifier::{ComponentState, ComponentStateOf, VerifyError, WithState};
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
///
///     /// Nothing is assumed open after a call, as at a function entry.
///     fn after_call(&mut self, _callee: Option<u32>, entry: &Self) {
///         self.clone_from(entry);
///     }
/// }
///
/// // Any machine state that contains `Open`: `Open` itself, or a
/// // `component_state!` struct with an `Open` field.
/// impl<S: ComponentStateOf<Open>> Channels for WithState<'_, S> {
///     fn open(&mut self, channel: u32) {
///         self.component_mut::<Open>().0.insert(channel);
///     }
///     fn require_open(&self, channel: u32) -> Result<(), VerifyError> {
///         if self.component::<Open>().0.contains(&channel) {
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

impl<S> WithState<'_, S> {
    /// The current state of the component whose state is `T`, inside the
    /// combined state `S` (see [`ComponentStateOf`]).
    pub fn component<T>(&self) -> &T
    where
        S: ComponentStateOf<T>,
    {
        self.state.component()
    }

    /// The current state of the component whose state is `T`, mutably.
    pub fn component_mut<T>(&mut self) -> &mut T
    where
        S: ComponentStateOf<T>,
    {
        self.state.component_mut()
    }
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
    #[allow(clippy::type_complexity)]
    pub fn verify<I>(
        &mut self,
        code: &[I],
        options: VerifyOptions,
    ) -> Result<Verification<(Frame<TypeSlot>, S)>, Rejection<(Frame<TypeSlot>, S)>>
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

impl<S: ComponentState> Signatures<Type> for WithState<'_, S> {
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

    /// Applies [`ComponentState::after_call`] to the component state.
    fn after_call(&mut self, callee: Option<u32>) {
        self.verifier.after_call(callee);
        self.state.after_call(callee, &self.entry);
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

impl<S> NamedTypes<Type> for WithState<'_, S> {
    fn named_type_count(&self) -> usize {
        self.verifier.named_type_count()
    }

    fn named_type_index(&self, name: &str) -> Result<u32, VerifyError> {
        self.verifier.named_type_index(name)
    }

    fn named_type_repr(&self, index: u32) -> Result<&Type, VerifyError> {
        self.verifier.named_type_repr(index)
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
        at: &JoinPoint,
    ) -> Result<bool, VerifyError> {
        // Join the component state on a copy first, so an error in either
        // join leaves the saved snapshot unchanged.
        let mut joined = state.clone();
        let state_changed = joined.join(in_state)?;
        let frame_changed = frame.join(in_frame, at)?;
        *state = joined;
        Ok(frame_changed || state_changed)
    }

    fn set_address(&mut self, function: u32, address: u32) {
        self.verifier.set_address(function, address);
    }
}
