use declarations::dialect;

pub type Parent = i64;
pub trait Kind {
    type Item;
}
pub struct Carrier;
impl Kind for Carrier {
    type Item = i64;
}
pub type A = u32;

#[dialect]
pub mod arithmetic {
    use super::Parent as Imported;
    pub type Local = i64;
    #[allow(non_camel_case_types)]
    pub type syntax = i64;
    pub struct Helper;

    /// Runtime operand path stays in this module.
    #[derive(Clone, Debug, PartialEq)]
    #[instruction]
    pub struct Add(pub self::Local);

    #[derive(Clone, Debug, PartialEq)]
    #[instruction]
    pub struct Symbol(pub framework::vihaco_parser_ident::Ident);

    #[derive(Clone, Debug, PartialEq)]
    #[instruction]
    pub struct Scoped(
        pub Imported,
        pub super::Parent,
        pub syntax,
        pub Vec<<super::Carrier as super::Kind>::Item>,
    );

    #[derive(Clone, Debug, PartialEq)]
    #[instruction]
    pub struct A(pub super::A);

    #[derive(Clone, Debug, PartialEq)]
    #[instruction]
    #[pattern = "'jump $0"]
    pub struct Branch(pub u32);
}

#[dialect]
pub mod control {
    #[derive(Clone, Debug, PartialEq)]
    #[instruction]
    pub struct Halt;
    #[derive(Clone, Debug, PartialEq)]
    #[instruction]
    pub struct Omitted;
}

pub mod left {
    #[declarations::dialect]
    pub mod cpu {
        #[instruction]
        pub struct Add;
    }
}
pub mod right {
    #[declarations::dialect]
    pub mod cpu {
        #[instruction]
        pub struct Halt;
    }
}

#[dialect]
pub mod r#type {
    #[instruction]
    pub struct Stop;
}

#[cfg(any())]
#[dialect]
mod disabled {
    #[instruction]
    enum Invalid {}
}

#[dialect]
pub mod many {
    #[instruction]
    pub struct V0;
    #[instruction]
    pub struct V1;
    #[instruction]
    pub struct V2;
    #[instruction]
    pub struct V3;
    #[instruction]
    pub struct V4;
    #[instruction]
    pub struct V5;
    #[instruction]
    pub struct V6;
    #[instruction]
    pub struct V7;
    #[instruction]
    pub struct V8;
    #[instruction]
    pub struct V9;
    #[instruction]
    pub struct V10;
    #[instruction]
    pub struct V11;
    #[instruction]
    pub struct V12;
    #[instruction]
    pub struct V13;
    #[instruction]
    pub struct V14;
    #[instruction]
    pub struct V15;
    #[instruction]
    pub struct V16;
    #[instruction]
    pub struct V17;
    #[instruction]
    pub struct V18;
    #[instruction]
    pub struct V19;
    #[instruction]
    pub struct V20;
    #[instruction]
    pub struct V21;
    #[instruction]
    pub struct V22;
    #[instruction]
    pub struct V23;
    #[instruction]
    pub struct V24;
    #[instruction]
    pub struct V25;
    #[instruction]
    pub struct V26;
    #[instruction]
    pub struct V27;
    #[instruction]
    pub struct V28;
    #[instruction]
    pub struct V29;
}
