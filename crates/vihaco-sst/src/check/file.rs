// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Checking a file whose sections target different machines.

use std::marker::PhantomData;

use vihaco_bytecode::{NoContext, SSTGlobalContext};

use super::{Block, Framing, ProgramChecker, Section, SectionChecker, check_file};
use crate::diagnostic::{Diagnostics, Report};
use crate::source::SourceFile;

/// Section checkers by section name, with an optional default for every
/// other section. Shared by [`FileChecker`] and the file formatter.
pub(crate) struct Routes<T: ?Sized> {
    sections: Vec<(String, Box<T>)>,
    default: Option<Box<T>>,
}

impl<T: ?Sized> Default for Routes<T> {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            default: None,
        }
    }
}

impl<T: ?Sized> Routes<T> {
    /// Route `name` to `handler`, replacing an earlier handler for it.
    pub(crate) fn insert(&mut self, name: String, handler: Box<T>) {
        self.sections.retain(|(existing, _)| *existing != name);
        self.sections.push((name, handler));
    }

    pub(crate) fn set_default(&mut self, handler: Box<T>) {
        self.default = Some(handler);
    }

    /// The handler for section `name`: its own, or the default.
    pub(crate) fn get_mut(&mut self, name: &str) -> Option<&mut T> {
        match self
            .sections
            .iter_mut()
            .find(|(existing, _)| existing == name)
        {
            Some((_, handler)) => Some(handler),
            None => self.default.as_deref_mut(),
        }
    }

    /// The names with their own handler, in registration order.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.sections.iter().map(|(name, _)| name.as_str())
    }

    fn all_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.sections
            .iter_mut()
            .map(|(_, handler)| &mut **handler)
            .chain(self.default.as_deref_mut())
    }
}

/// Anything routed that can check a section.
pub(crate) trait AsSectionChecker {
    fn as_checker(&mut self) -> &mut dyn SectionChecker;
}

impl AsSectionChecker for dyn SectionChecker {
    fn as_checker(&mut self) -> &mut dyn SectionChecker {
        self
    }
}

/// A [`SectionChecker`] that checks the `.global` block as `C` and each
/// section with its routed checker.
pub(crate) struct Routed<'r, T: ?Sized, C> {
    pub(crate) routes: &'r mut Routes<T>,
    pub(crate) global: PhantomData<fn() -> C>,
}

impl<T, C> SectionChecker for Routed<'_, T, C>
where
    T: AsSectionChecker + ?Sized,
    C: SSTGlobalContext,
{
    fn check_global(&mut self, global: &Block<'_>, out: &mut Diagnostics<'_>) {
        super::check_global_context::<C>(global, global.span(), out);
    }

    fn check_section(&mut self, section: &Section<'_>, out: &mut Diagnostics<'_>) {
        match self.routes.get_mut(section.name()) {
            Some(handler) => handler.as_checker().check_section(section, out),
            None => {
                let names: Vec<String> = self
                    .routes
                    .names()
                    .map(|name| format!("`{name}`"))
                    .collect();
                let mut diagnostic = out
                    .build_error(
                        section.name_span(),
                        format!("no machine is registered for section `{}`", section.name()),
                    )
                    .with_label("unknown section");
                if !names.is_empty() {
                    diagnostic =
                        diagnostic.with_note(format!("registered sections: {}", names.join(", ")));
                }
                out.push(diagnostic);
            }
        }
    }

    fn finish(&mut self, framing: &Framing<'_>, out: &mut Diagnostics<'_>) {
        let missing: Vec<String> = self
            .routes
            .names()
            .filter(|name| framing.section(name).is_none())
            .map(str::to_owned)
            .collect();
        for name in missing {
            out.error(framing.version, format!("missing section `{name}`"), "");
        }
        for handler in self.routes.all_mut() {
            handler.as_checker().finish(framing, out);
        }
    }
}

/// Checks a file whose sections may target different machines.
///
/// Each section is checked by the checker registered for its name, or by
/// the default checker if it has none; a section with neither is an error,
/// as is a registered section that the file does not contain. The `.global`
/// block is checked as `C` ([`NoContext`] by default), once for the whole
/// file; the section checkers' own `check_global` is not called.
///
/// ```
/// use vihaco_sst::{FileChecker, SourceFile};
/// # use vihaco_sst::{Diagnostics, check::Section};
/// # let controller = |_: &Section<'_>, _: &mut Diagnostics<'_>| {};
/// # let fpga = |_: &Section<'_>, _: &mut Diagnostics<'_>| {};
///
/// // With machines: `.section::<Controller>("controller").section::<Fpga>("fpga")`.
/// let mut checker = FileChecker::new()
///     .section_with("controller", controller)
///     .section_with("fpga", fpga);
/// let source = SourceFile::new("p.sst", "sst v2\n.section controller\n.section dsp\n");
/// let messages: Vec<String> = checker.check(&source).iter().map(ToString::to_string).collect();
/// assert_eq!(messages, [
///     "1:1: error: missing section `fpga`",
///     "3:10: error: no machine is registered for section `dsp`",
/// ]);
/// ```
pub struct FileChecker<C = NoContext> {
    routes: Routes<dyn SectionChecker>,
    global: PhantomData<fn() -> C>,
}

impl FileChecker {
    /// A checker with no sections registered, for a file without a
    /// `.global` context.
    pub fn new() -> Self {
        Self {
            routes: Routes::default(),
            global: PhantomData,
        }
    }
}

impl Default for FileChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> FileChecker<C> {
    /// Check section `name` as a program for machine `M`, with the standard
    /// checker `ProgramChecker::<M>::new()`. The section is required.
    #[must_use]
    pub fn section<M: 'static>(self, name: impl Into<String>) -> Self
    where
        ProgramChecker<M>: SectionChecker,
    {
        self.section_with(name, ProgramChecker::<M>::new())
    }

    /// Check section `name` with `checker`, for example a [`ProgramChecker`]
    /// with an application header. The section is required.
    #[must_use]
    pub fn section_with(
        mut self,
        name: impl Into<String>,
        checker: impl SectionChecker + 'static,
    ) -> Self {
        self.routes.insert(name.into(), Box::new(checker));
        self
    }

    /// Check every section without its own checker as a program for `M`.
    #[must_use]
    pub fn default_section<M: 'static>(self) -> Self
    where
        ProgramChecker<M>: SectionChecker,
    {
        self.default_with(ProgramChecker::<M>::new())
    }

    /// Check every section without its own checker with `checker`.
    #[must_use]
    pub fn default_with(mut self, checker: impl SectionChecker + 'static) -> Self {
        self.routes.set_default(Box::new(checker));
        self
    }

    /// Parse the `.global` block as `C2`.
    pub fn with_global_context<C2>(self) -> FileChecker<C2> {
        FileChecker {
            routes: self.routes,
            global: PhantomData,
        }
    }

    /// Check a whole file.
    pub fn check(&mut self, source: &SourceFile) -> Report
    where
        C: SSTGlobalContext,
    {
        check_file(
            source,
            &mut Routed::<_, C> {
                routes: &mut self.routes,
                global: PhantomData,
            },
        )
    }
}
