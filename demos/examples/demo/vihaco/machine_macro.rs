// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// The planned `machine!` macro will make effect fanout explicit in each runtime instruction arm:
//
//     effects {
//         observe foo, bar;
//         to foobar;
//     }
//
// It will generate calls to every listed observer followed by exactly one call to the listed
// handler. Observers borrow the effect; the handler receives ownership.
