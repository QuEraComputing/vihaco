// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

/// Compile-time identity for one instruction route selected by a composite.
///
/// A component can implement the same instruction or effect type in several places. The route
/// marker keeps those selections distinct: `IntegerAdd` and `IntegerSub`, for example, may both
/// produce `ValueResult`, but they are still different routes with independently generated
/// message, effect, timing, and diagnostic wiring. This is also why a route marker is separate
/// from a runtime completion state: this trait describes the static dispatch path, while runtime
/// execution state describes the operation that is currently running or parked.
///
/// The composite machinery generates one marker and one implementation for every selected route.
/// Users provide the component operations and handlers; they do not implement this trait.
pub trait Route {
    /// Effect produced by the component on this route and passed to its observers and handlers.
    ///
    /// The association lets generated dispatch name the route once and derive the effect type
    /// from it, rather than repeating that type throughout every generated call site.
    type Effect;

    /// Error type used to normalize failures at this route's dispatch boundary.
    ///
    /// Component execution, message resolution, observation, and effect handling may each have
    /// their own lower-level errors. Generated wiring converts those errors into the route's
    /// containing-machine error type before returning from dispatch.
    type Error;
}
