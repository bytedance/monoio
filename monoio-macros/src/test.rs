use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

pub(crate) fn test_if_support_arch(func: TokenStream) -> TokenStream {
    let func = parse_macro_input!(func as ItemFn);
    let func_vis = &func.vis; // like pub
    let func_block = &func.block; // { some statement or expression here }

    let func_decl = func.sig;
    let func_name = &func_decl.ident; // function name
    let func_generics = &func_decl.generics;
    let func_inputs = &func_decl.inputs;
    let func_output = &func_decl.output;

    // rebuild the function
    let caller = quote! {
        // some test report { code: 38, kind: Unsupported, message: "Function not implemented" } in aarch64,
        // armv7, riscv64gc, s390x, ignore these tests in the archs
        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "riscv64",
            target_arch = "s390x",
        )))]
        #func_vis fn #func_name #func_generics(#func_inputs) #func_output {
            #func_block
        }
    };
    caller.into()
}
