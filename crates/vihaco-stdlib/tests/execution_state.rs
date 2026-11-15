// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

#![cfg(feature = "program-image-execution")]

use vihaco_abi::{traits::Reset, word::Word};
use vihaco_module::{
    PopStack, PushStack,
    module::{FunctionInfo, LocalModule, Signature},
};

use vihaco_stdlib::ExecutionState;

fn module() -> LocalModule<(), (), (), ()> {
    LocalModule {
        code: vec![()],
        structs: vec![],
        types: vec![],
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![vihaco_module::module::Parameter { name: 0, ty: () }],
                ret: vec![],
            },
            locals: vec![(), ()],
            start_address: 0,
            end_address: 0,
            file: 0,
        }],
        main_function: Some(0),
        ..Default::default()
    }
}

#[test]
fn entry_isolates_locals_and_reset_reuses_state() -> eyre::Result<()> {
    let module = module();
    let mut stack = ExecutionState::new();
    stack.start(&module, &[7])?;
    assert_eq!(stack.stack(), &[7, 0]);
    assert_eq!(stack.frames().len(), 1);
    assert!(<ExecutionState as PopStack<Word>>::pop(&mut stack).is_err());
    <ExecutionState as PushStack<Word>>::push(&mut stack, 9);
    assert_eq!(<ExecutionState as PopStack<Word>>::pop(&mut stack)?, 9);
    assert!(stack.start(&module, &[8]).is_err());
    stack.reset();
    assert!(stack.is_halted());
    assert!(stack.frames().is_empty());
    stack.start(&module, &[8])?;
    assert_eq!(stack.stack(), &[8, 0]);
    Ok(())
}

#[test]
fn invalid_entry_preserves_state() {
    let mut module = module();
    let mut stack = ExecutionState::new();
    // Unnamed functions are shown by index; named ones as `@main`.
    assert_eq!(
        stack.start(&module, &[]).unwrap_err().to_string(),
        "function 0 takes 1 argument (_), 0 given"
    );
    module.strings = vec!["main".into(), "p0".into()];
    module.functions[0].signature.params[0].name = 1;
    assert_eq!(
        stack.start(&module, &[1, 2]).unwrap_err().to_string(),
        "@main takes 1 argument (p0), 2 given"
    );
    module.functions[0].locals.clear();
    assert!(stack.start(&module, &[7]).is_err());
    module.functions[0].locals = vec![(), ()];
    module.functions[0].start_address = 1;
    assert!(stack.start(&module, &[7]).is_err());
    assert!(stack.is_halted());
    assert!(stack.stack().is_empty());
    assert!(stack.frames().is_empty());
}

#[test]
fn oversized_frames_are_rejected_before_allocating() {
    let mut module = module();
    // Zero-sized local types make a huge declared frame free to build; without
    // the cap, entry would try to reserve `usize::MAX` words.
    module.functions[0].locals = vec![(); usize::MAX];
    let mut stack = ExecutionState::new();
    let err = stack.start(&module, &[7]).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "function 0 declares {} locals, more than the maximum of 65536",
            usize::MAX
        )
    );
    module.strings = vec!["main".into()];
    let err = stack.start(&module, &[7]).unwrap_err();
    assert!(
        err.to_string().starts_with("`@main` (function 0) declares"),
        "{err}"
    );
    assert!(stack.is_halted());
    assert!(stack.stack().is_empty());
    assert!(stack.frames().is_empty());
}
