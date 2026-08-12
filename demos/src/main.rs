// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

//! Earlier demo for the rewrite, keeping for "historical purposes"
//! see examples/ for a working demo

#![allow(warnings)]

fn main() {}

macro_rules! machine {
    ($($tokens:tt)*) => {};
}

macro_rules! use_as_vihaco {
    () => {
        use crate::vihaco_concepts as vihaco;
    };
}

macro_rules! use_vihaco_parse {
    () => {
        #[allow(unused_imports)]
        use vihaco_parser::Parse;
        use vihaco_parser_derive::Parse;
    };
}

mod vihaco_concepts {
    use vihaco::Effects;

    pub struct NoMessage {}
    pub enum NoFault {}
    pub enum NoEffect {}

    pub trait Type {}

    pub trait Value {
        type Type: Type;

        fn type_of(&self) -> Self::Type;
    }

    pub struct BinaryOperands<V> {
        pub lhs: V,
        pub rhs: V,
    }

    pub trait Execute<I> {
        type Message;
        type Effect;
        type Fault;

        fn execute(
            &mut self,
            instruction: I,
            message: Self::Message,
        ) -> Result<Effects<Self::Effect>, Self::Fault>;
    }

    pub enum Execution {
        Continue,
        Parked,
    }

    pub trait Step {
        type Instruction;
        type Fault;

        fn step(&mut self, instruction: Self::Instruction) -> Result<Execution, Self::Fault>;
    }
}

mod machine {
    use crate::*;

    pub struct Composite {
        clock: clock::GlobalClock,
        channels: channel::ChannelManager,
        cpu_a: cpu::CPU,
        cpu_b: cpu::CPU,
    }

    mod syntax {
        pub enum Instruction {}
    }

    mod instruction {
        pub enum Instruction {}
    }
}

mod channel {
    use crate::*;
    use_as_vihaco!();

    struct Channel {
        sender: u32,
        receiver: u32,
    }

    pub struct MessageChannel {}

    pub struct ChannelManager {
        channels: Vec<Channel>,
    }

    pub mod syntax {
        use_vihaco_parse!();

        #[derive(Parse)]
        #[syntax_class(instruction)]
        pub enum Instruction {
            #[pattern = "'channel::send $0"]
            Send(u32),
            #[pattern = "'channel::recv $0"]
            Recv(u32),
        }

        pub struct Send {}
        pub struct Recv {}
    }

    pub mod instruction {
        use super::*;

        pub enum Instruction {
            Send(Send),
            Recv(Recv),
        }

        pub struct Send {
            channel: u32,
        }

        pub enum SendFault {
            ChannelDoesNotExist,
        }

        pub struct Recv {
            channel: u32,
        }

        impl vihaco::Execute<Send> for ChannelManager {
            type Message = vihaco::NoMessage;
            type Effect = vihaco::NoEffect;
            type Fault = SendFault;

            fn execute(
                &mut self,
                instruction: Send,
                message: Self::Message,
            ) -> Result<::vihaco::Effects<Self::Effect>, Self::Fault> {
                todo!()
            }
        }
    }
}

mod cpu {
    use crate::*;
    use_as_vihaco!();

    enum Type {
        U32,
    }

    impl vihaco::Type for Type {}

    enum Value {
        U32(u32),
    }

    impl vihaco::Value for Value {
        type Type = Type;

        fn type_of(&self) -> Self::Type {
            match self {
                Self::U32(..) => Type::U32,
            }
        }
    }

    machine!(
        composite CPU {
            alu: arithmetic::ALU<Value>,
            stack: stack::Stack<Value>,
            clock: clock::LocalClock,
            channels: channel::MessageChannel,
        }

        syntax {
            Add <= arithmetic::syntax::Add,
            Sub <= arithmetic::syntax::Sub,
            Mul <= arithmetic::syntax::Mul,
            Send <= channel::syntax::Send,
            Recv <= channel::syntax::Recv,
        }

        runtime {
            ![0x01]
            Add => arithmetic::instruction::Add {
                message from stack;
                effects to stack;
            }

            ![0x02]
            Sub => arithmetic::instruction::Sub {
                message from stack;
                effects to stack;
            }

            ![0x03]
            Mul => arithmetic::instruction::Mul {
                message from stack;
                effects to stack;
            }

            ![0x04]
            Send => channel::instruction::Send {
                message from channels;
                effects to channels, clock;
            }

            ![0x05]
            Recv => channel::instruction::Send {
                message from channels;
                effects to channels, clock;
            }
        }
    );

    pub struct CPU {
        alu: arithmetic::ALU<Value>,
        stack: stack::Stack<Value>,
        clock: clock::LocalClock,
        channels: channel::MessageChannel,
    }

    pub enum CPUInstruction {
        Add(arithmetic::instruction::Add<Type>),
        Sub(arithmetic::instruction::Sub<Type>),
        Mul(arithmetic::instruction::Mul<Type>),
        Send(channel::instruction::Send),
        Recv(channel::instruction::Recv),
    }
}

mod stack {
    pub struct Stack<T> {
        stack: Vec<T>,
    }

    impl<T> Stack<T> {
        pub fn push(&mut self, value: T) {
            self.stack.push(value);
        }

        pub fn pop(&mut self) -> Option<T> {
            self.stack.pop()
        }
    }
}

mod clock {
    struct Timeline {}

    pub struct GlobalClock {
        timeline: Timeline,
    }

    pub struct LocalClock {}
}

mod arithmetic {
    use std::marker::PhantomData;
    use_as_vihaco!();

    pub struct ALU<V: vihaco::Value> {
        _marker: PhantomData<fn() -> V>,
    }

    pub mod syntax {
        use_vihaco_parse!();

        #[derive(Parse)]
        #[syntax_class(instruction)]
        pub enum Instruction<Ty>
        where
            Ty: for<'a> Parse<'a>,
        {
            #[pattern = "'arith::add $0"]
            Add(Ty),
            #[pattern = "'arith::sub $0"]
            Sub(Ty),
            #[pattern = "'arith::mul $0"]
            Mul(Ty),
        }

        pub struct Add {}
        pub struct Sub {}
        pub struct Mul {}
    }

    pub mod instruction {
        use super::*;
        use ::vihaco::effect::Effects;

        pub enum Instruction<Ty> {
            Add(Add<Ty>),
            Sub(Sub<Ty>),
            Mul(Mul<Ty>),
        }

        pub struct Add<Ty> {
            ty: Ty,
        }

        pub trait TryAdd: vihaco::Value {
            type Result;
            type Fault;

            fn try_add(&self, other: Self, ty: Self::Type) -> Result<Self::Result, Self::Fault>;
        }

        impl<V> vihaco::Execute<Add<V::Type>> for ALU<V>
        where
            V: TryAdd,
        {
            type Message = vihaco::BinaryOperands<V>;
            type Effect = effect::ValueResult<V::Result>;
            type Fault = V::Fault;

            fn execute(
                &mut self,
                instruction: Add<V::Type>,
                message: Self::Message,
            ) -> Result<Effects<Self::Effect>, Self::Fault> {
                let value = message.lhs.try_add(message.rhs, instruction.ty)?;
                Ok(Effects::One(effect::ValueResult { value }))
            }
        }

        pub struct Sub<Ty> {
            ty: Ty,
        }

        pub trait TrySub: vihaco::Value {
            type Result;
            type Fault;

            fn try_sub(&self, other: Self, ty: Self::Type) -> Result<Self::Result, Self::Fault>;
        }

        impl<V> vihaco::Execute<Sub<V::Type>> for ALU<V>
        where
            V: TrySub,
        {
            type Message = vihaco::BinaryOperands<V>;
            type Effect = effect::ValueResult<V::Result>;
            type Fault = V::Fault;

            fn execute(
                &mut self,
                instruction: Sub<V::Type>,
                message: Self::Message,
            ) -> Result<Effects<Self::Effect>, Self::Fault> {
                let value = message.lhs.try_sub(message.rhs, instruction.ty)?;
                Ok(Effects::One(effect::ValueResult { value }))
            }
        }

        pub struct Mul<Ty> {
            ty: Ty,
        }

        pub trait TryMul: vihaco::Value {
            type Result;
            type Fault;

            fn try_mul(&self, other: Self, ty: Self::Type) -> Result<Self::Result, Self::Fault>;
        }

        impl<V> vihaco::Execute<Mul<V::Type>> for ALU<V>
        where
            V: TryMul,
        {
            type Message = vihaco::BinaryOperands<V>;
            type Effect = effect::ValueResult<V::Result>;
            type Fault = V::Fault;

            fn execute(
                &mut self,
                instruction: Mul<V::Type>,
                message: Self::Message,
            ) -> Result<Effects<Self::Effect>, Self::Fault> {
                let value = message.lhs.try_mul(message.rhs, instruction.ty)?;
                Ok(Effects::One(effect::ValueResult { value }))
            }
        }
    }

    mod effect {
        pub struct ValueResult<V> {
            pub value: V,
        }
    }
}
