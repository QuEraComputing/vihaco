mod attr_component;
mod attr_observe;
mod common;
mod derive_instruction;
mod derive_machine;
mod derive_message;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

#[proc_macro_derive(Instruction, attributes(instruction, opcode))]
pub fn derive_instruction(input: TokenStream) -> TokenStream {
    derive_instruction::expand(input)
}

#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
    derive_message::expand(input)
}

#[proc_macro_derive(Machine, attributes(device, observe, program))]
pub fn derive_machine(input: TokenStream) -> TokenStream {
    derive_machine::expand(input)
}

#[proc_macro_attribute]
pub fn composite(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let original = proc_macro2::TokenStream::from(item.clone());
    let generated = proc_macro2::TokenStream::from(derive_machine::expand(item));
    let mut sanitized: DeriveInput = match syn::parse2(original) {
        Ok(input) => input,
        Err(err) => return err.into_compile_error().into(),
    };

    if let Data::Struct(data) = &mut sanitized.data
        && let Fields::Named(fields) = &mut data.fields
    {
        for field in &mut fields.named {
            field.attrs.retain(|attr| {
                let path = attr.path();
                !(path.is_ident("device") || path.is_ident("observe") || path.is_ident("program"))
            });
        }
    }

    quote! {
        #sanitized
        #generated
    }
    .into()
}

#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_component::expand(attr, item)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn observe(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr_observe::expand(attr, item)
}
