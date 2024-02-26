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
#[proc_macro_attribute]
pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    entry::main(args, item)
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
