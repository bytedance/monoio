// Heavily borrowed from tokio.
// Copyright (c) 2021 Tokio Contributors, licensed under the MIT license.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, quote_spanned, ToTokens};

struct FinalConfig {
    entries: Option<u32>,
    timer_enabled: Option<bool>,
    threads: Option<u32>,
    driver: DriverType,
}

struct Configuration {
    entries: Option<(u32, Span)>,
    timer_enabled: Option<(bool, Span)>,
    threads: Option<(u32, Span)>,
    driver: Option<(DriverType, Span)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DriverType {
    Legacy,
    Uring,
    Fusion,
}

impl Configuration {
    fn new() -> Self {
        Configuration {
            entries: None,
            timer_enabled: None,
            threads: None,
            driver: None,
        }
    }

    fn set_driver(&mut self, driver: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.driver.is_some() {
            return Err(syn::Error::new(span, "`driver` set multiple times."));
        }

        let driver = parse_driver(driver, span, "driver")?;
        self.driver = Some((driver, span));
        Ok(())
    }

    fn set_threads(&mut self, threads: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.threads.is_some() {
            return Err(syn::Error::new(span, "`threads` set multiple times."));
        }

        let threads = parse_int(threads, span, "threads")? as u32;
        if threads == 0 {
            return Err(syn::Error::new(span, "`threads` may not be 0."));
        }
        self.threads = Some((threads, span));
        Ok(())
    }

    fn set_entries(&mut self, entries: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.entries.is_some() {
            return Err(syn::Error::new(span, "`entries` set multiple times."));
        }

        let entries = parse_int(entries, span, "entries")? as u32;
        if entries == 0 {
            return Err(syn::Error::new(span, "`entries` may not be 0."));
        }
        self.entries = Some((entries, span));
        Ok(())
    }

    fn set_timer_enabled(&mut self, enabled: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.timer_enabled.is_some() {
            return Err(syn::Error::new(span, "`timer_enabled` set multiple times."));
        }

        let enabled = parse_bool(enabled, span, "timer_enabled")?;
        self.timer_enabled = Some((enabled, span));
        Ok(())
    }

    fn build(&self) -> Result<FinalConfig, syn::Error> {
        Ok(FinalConfig {
            entries: self.entries.map(|(e, _)| e),
            timer_enabled: self.timer_enabled.map(|(t, _)| t),
            threads: self.threads.map(|(t, _)| t),
            driver: self.driver.map(|(d, _)| d).unwrap_or(DriverType::Fusion),
        })
    }
}

#[allow(unused)]
fn parse_int(int: syn::Lit, span: Span, field: &str) -> Result<usize, syn::Error> {
    match int {
        syn::Lit::Int(lit) => match lit.base10_parse::<usize>() {
            Ok(value) => Ok(value),
            Err(e) => Err(syn::Error::new(
                span,
                format!("Failed to parse value of `{field}` as integer: {e}"),
            )),
        },
        _ => Err(syn::Error::new(
            span,
            format!("Failed to parse value of `{field}` as integer."),
        )),
    }
}

#[allow(unused)]
fn parse_string(lit: syn::Lit, span: Span, field: &str) -> Result<String, syn::Error> {
    match lit {
        syn::Lit::Str(s) => Ok(s.value()),
        syn::Lit::Verbatim(s) => Ok(s.to_string()),
        _ => Err(syn::Error::new(
            span,
            format!("Failed to parse value of `{field}` as string."),
        )),
    }
}

#[allow(unused)]
fn parse_driver(lit: syn::Lit, span: Span, field: &str) -> Result<DriverType, syn::Error> {
    let val = parse_string(lit, span, field)?;
    match val.as_str() {
        "legacy" => Ok(DriverType::Legacy),
        "uring" | "io_uring" | "iouring" => Ok(DriverType::Uring),
        "fusion" | "auto" | "detect" => Ok(DriverType::Fusion),
        _ => Err(syn::Error::new(
            span,
            format!("Failed to parse value of `{field}` as DriverType."),
        )),
    }
}

#[allow(unused)]
fn parse_bool(bool: syn::Lit, span: Span, field: &str) -> Result<bool, syn::Error> {
    match bool {
        syn::Lit::Bool(b) => Ok(b.value),
        _ => Err(syn::Error::new(
            span,
            format!("Failed to parse value of `{field}` as bool."),
        )),
    }
}

fn parse_knobs(
    mut input: syn::ItemFn,
    args: syn::AttributeArgs,
    is_test: bool,
    mut config: Configuration,
) -> Result<TokenStream, syn::Error> {
    if input.sig.asyncness.take().is_none() {
        let msg = "the `async` keyword is missing from the function declaration";
        return Err(syn::Error::new_spanned(input.sig.fn_token, msg));
    }

    for arg in args {
        match arg {
            syn::NestedMeta::Meta(syn::Meta::NameValue(namevalue)) => {
                let ident = namevalue
                    .path
                    .get_ident()
                    .ok_or_else(|| {
                        syn::Error::new_spanned(&namevalue, "Must have specified ident")
                    })?
                    .to_string()
                    .to_lowercase();
                match ident.as_str() {
                    "entries" => config.set_entries(
                        namevalue.lit.clone(),
                        syn::spanned::Spanned::span(&namevalue.lit),
                    )?,
                    "timer_enabled" | "enable_timer" | "timer" => config.set_timer_enabled(
                        namevalue.lit.clone(),
                        syn::spanned::Spanned::span(&namevalue.lit),
                    )?,
                    "worker_threads" | "workers" | "threads" => config.set_threads(
                        namevalue.lit.clone(),
                        syn::spanned::Spanned::span(&namevalue.lit),
                    )?,
                    "driver" => config.set_driver(
                        namevalue.lit.clone(),
                        syn::spanned::Spanned::span(&namevalue.lit),
                    )?,
                    name => {
                        let msg = format!(
                            "Unknown attribute {} is specified; expected one of: \
                             `worker_threads`, `entries`, `timer_enabled`",
                            name,
                        );
                        return Err(syn::Error::new_spanned(namevalue, msg));
                    }
                }
            }
            syn::NestedMeta::Meta(syn::Meta::Path(path)) => {
                let name = path
                    .get_ident()
                    .ok_or_else(|| syn::Error::new_spanned(&path, "Must have specified ident"))?
                    .to_string()
                    .to_lowercase();
                let msg = format!(
                    "Unknown attribute {} is specified; expected one of: `worker_threads`, \
                     `entries`, `timer_enabled`",
                    name
                );
                return Err(syn::Error::new_spanned(path, msg));
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "Unknown attribute inside the macro",
                ));
            }
        }
    }

    let config = config.build()?;

    // If type mismatch occurs, the current rustc points to the last statement.
    let (last_stmt_start_span, last_stmt_end_span) = {
        let mut last_stmt = input
            .block
            .stmts
            .last()
            .map(ToTokens::into_token_stream)
            .unwrap_or_default()
            .into_iter();
        // `Span` on stable Rust has a limitation that only points to the first
        // token, not the whole tokens. We can work around this limitation by
        // using the first/last span of the tokens like
        // `syn::Error::new_spanned` does.
        let start = last_stmt.next().map_or_else(Span::call_site, |t| t.span());
        let end = last_stmt.last().map_or(start, |t| t.span());
        (start, end)
    };

    let mut rt = match config.driver {
        DriverType::Legacy => {
            quote_spanned! {last_stmt_start_span=>monoio::RuntimeBuilder::<monoio::LegacyDriver>::new()}
        }
        DriverType::Uring => {
            quote_spanned! {last_stmt_start_span=>monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()}
        }
        DriverType::Fusion => {
            quote_spanned! {last_stmt_start_span=>monoio::RuntimeBuilder::<monoio::FusionDriver>::new()}
        }
    };

    if let Some(entries) = config.entries {
        rt = quote! { #rt.with_entries(#entries) }
    }
    if Some(true) == config.timer_enabled {
        rt = quote! { #rt.enable_timer() }
    }

    let body = &input.block;
    let brace_token = input.block.brace_token;
    let (tail_return, tail_semicolon) = match body.stmts.last() {
        Some(syn::Stmt::Semi(expr, _)) => (
            match expr {
                syn::Expr::Return(_) => quote! { return },
                _ => quote! {},
            },
            quote! {
                ;
            },
        ),
        _ => (quote! {}, quote! {}),
    };

    if matches!(config.threads, None | Some(1)) {
        input.block = syn::parse2(quote_spanned! {last_stmt_end_span=>
            {
                let body = async #body;
                #[allow(clippy::expect_used)]
                #tail_return #rt
                    .build()
                    .expect("Failed building the Runtime")
                    .block_on(body)#tail_semicolon
            }
        })
        .expect("Parsing failure");
    } else {
        // Function must return `()` since it will be swallowed.
        if !matches!(input.sig.output, syn::ReturnType::Default) {
            return Err(syn::Error::new(
                last_stmt_end_span,
                "With multi-thread function can not have a return value",
            ));
        }

        let threads = config.threads.unwrap() - 1;
        input.block = syn::parse2(quote_spanned! {last_stmt_end_span=>
            {
                let body = async #body;

                #[allow(clippy::needless_collect)]
                let threads: Vec<_> = (0 .. #threads)
                    .map(|_| {
                        ::std::thread::spawn(|| {
                            #rt.build()
                                .expect("Failed building the Runtime")
                                .block_on(async #body);
                        })
                    })
                    .collect();
                // Run on main threads
                #rt.build()
                    .expect("Failed building the Runtime")
                    .block_on(body);

                // Wait for other threads
                threads.into_iter().for_each(|t| {
                    let _ = t.join();
                });
            }
        })
        .expect("Parsing failure");
    }

    input.block.brace_token = brace_token;

    let header = if is_test {
        quote! {
            #[::core::prelude::v1::test]
        }
    } else {
        quote! {}
    };
    let cfg_attr = if config.driver == DriverType::Uring && is_test {
        quote! {
            #[cfg(target_os = "linux")]
        }
    } else {
        quote! {}
    };
    let result = quote! {
        #header
        #cfg_attr
        #input
    };
    Ok(result.into())
}

pub(crate) fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);

    if input.sig.ident == "main" && !input.sig.inputs.is_empty() {
        let msg = "the main function cannot accept arguments";
        return syn::Error::new_spanned(&input.sig.ident, msg)
            .to_compile_error()
            .into();
    }

    parse_knobs(input, args, false, Configuration::new())
        .unwrap_or_else(|e| e.to_compile_error().into())
}

pub(crate) fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);

    for attr in &input.attrs {
        if attr.path.is_ident("test") {
            let msg = "second test attribute is supplied";
            return syn::Error::new_spanned(attr, msg).to_compile_error().into();
        }
    }

    parse_knobs(input, args, true, Configuration::new())
        .unwrap_or_else(|e| e.to_compile_error().into())
}

pub(crate) fn test_all(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);

    for attr in &input.attrs {
        if attr.path.is_ident("test") {
            let msg = "second test attribute is supplied";
            return syn::Error::new_spanned(attr, msg).to_compile_error().into();
        }
    }

    let mut input_uring = input.clone();
    input_uring.sig.ident = proc_macro2::Ident::new(
        &format!("uring_{}", input_uring.sig.ident),
        input_uring.sig.ident.span(),
    );
    let mut config = Configuration::new();
    config.driver = Some((DriverType::Uring, Span::call_site()));
    let mut token_uring = parse_knobs(input_uring, args.clone(), true, config)
        .unwrap_or_else(|e| e.to_compile_error().into());

    let mut input_legacy = input;
    input_legacy.sig.ident = proc_macro2::Ident::new(
        &format!("legacy_{}", input_legacy.sig.ident),
        input_legacy.sig.ident.span(),
    );
    let mut config = Configuration::new();
    config.driver = Some((DriverType::Legacy, Span::call_site()));
    let token_legacy = parse_knobs(input_legacy, args, true, config)
        .unwrap_or_else(|e| e.to_compile_error().into());
    token_uring.extend(token_legacy);
    token_uring
}
