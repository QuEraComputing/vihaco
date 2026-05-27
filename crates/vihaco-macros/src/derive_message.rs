use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn expand(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    quote! {
        impl ::vihaco::runtime::Message for #ident {}
    }
    .into()
}
