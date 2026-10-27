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
        functions: vec![FunctionInfo {
            name: 0,
            signature: Signature {
                params: vec![vihaco_module::module::Parameter { name: 0, ty: () }],
                ret: vec![],
            },
            local_count: 2,
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
    assert!(stack.start(&module, &[]).is_err());
    module.functions[0].local_count = 0;
    assert!(stack.start(&module, &[7]).is_err());
    module.functions[0].local_count = 2;
    module.functions[0].start_address = 1;
    assert!(stack.start(&module, &[7]).is_err());
    assert!(stack.is_halted());
    assert!(stack.stack().is_empty());
    assert!(stack.frames().is_empty());
}
