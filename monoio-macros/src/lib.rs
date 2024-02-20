#![allow(clippy::needless_doctest_main)]
//! Macros for use with Monoio

// This `extern` is required for older `rustc` versions but newer `rustc`
// versions warn about the unused `extern crate`.
// Copyright (c) 2021 Tokio Contributors, licensed under the MIT license.
#[allow(unused_extern_crates)]
extern crate proc_macro;

mod entry;
mod select;

use proc_macro::TokenStream;

#[cfg(unix)]
#[proc_macro_attribute]
pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    entry::main(args, item)
}

#[cfg(windows)]
#[proc_macro_attribute]
pub fn main(_args: TokenStream, func: TokenStream) -> TokenStream {
    use quote::quote;
    use syn::parse_macro_input;

    let func = parse_macro_input!(func as syn::ItemFn);
    let func_vis = &func.vis; // like pub

    let func_decl = func.sig;
    let func_name = &func_decl.ident; // function name
    let func_generics = &func_decl.generics;
    let func_inputs = &func_decl.inputs;
    let _func_output = &func_decl.output;

    let caller = quote! {
        // rebuild the function
        #func_vis fn #func_name #func_generics(#func_inputs) {
            println!("macros unimplemented in windows!");
        }
    };
    caller.into()
}

#[proc_macro_attribute]
pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    entry::test(args, item)
}

#[proc_macro_attribute]
pub fn test_all(args: TokenStream, item: TokenStream) -> TokenStream {
    entry::test_all(args, item)
}

/// Implementation detail of the `select!` macro. This macro is **not** intended
/// to be used as part of the public API and is permitted to change.
#[proc_macro]
#[doc(hidden)]
pub fn select_priv_declare_output_enum(input: TokenStream) -> TokenStream {
    select::declare_output_enum(input)
}
