//! Proc macros for the Morrow modding SDK.
//!
//! - `#[morrow::mod_main]` — marks the mod entry point.
//! - `#[morrow::event(kind)]` — registers an event handler.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse_macro_input, FnArg, Ident, ItemFn, PatType, ReturnType, Type, TypePath, TypeReference,
};

// ---------------------------------------------------------------------------
// #[morrow::mod_main]
// ---------------------------------------------------------------------------

/// Marks a function as the entry point for a Morrow mod.
///
/// The function must have one of these signatures:
/// `fn(&mut Context) -> Result<(), MorrowError>` (recommended)
/// `fn(&mut Context, *const RuntimeApi) -> Result<(), MorrowError>` (legacy)
///
/// The macro generates `morrow_mod_init(api: *const RuntimeApi) -> u32`
/// which the runtime calls. The init body runs inside `catch_unwind` so a
/// panicking mod fails cleanly (error logged, code 1) instead of aborting
/// across the FFI boundary.
///
/// # Example
///
/// ```ignore
/// use morrow::prelude::*;
///
/// #[morrow::mod_main]
/// fn init(ctx: &mut Context) -> Result<(), MorrowError> {
///     morrow::info!("Hello from my mod!");
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn mod_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    match expand_mod_main(input) {
        Ok(ts) => ts,
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_mod_main(input: ItemFn) -> syn::Result<TokenStream> {
    let fn_name = &input.sig.ident;
    let vis = &input.vis;
    let original = &input;

    // Validate signature
    let args: Vec<&FnArg> = input.sig.inputs.iter().collect();
    if args.first().is_some_and(|a| matches!(a, FnArg::Receiver(_))) {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "#[morrow::mod_main] cannot be used on a method (no `self`)",
        ));
    }
    if !input.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.sig.generics,
            "#[morrow::mod_main] function must not be generic",
        ));
    }

    // Arity 1: fn(&mut Context) -> Result<..>  (recommended)
    // Arity 2: fn(&mut Context, *const RuntimeApi) -> Result<..>  (legacy)
    let legacy = match args.len() {
        1 => false,
        2 => true,
        n => {
            return Err(syn::Error::new_spanned(
                &input.sig,
                format!(
                    "#[morrow::mod_main] function must take `&mut Context` \
                     (or legacy `&mut Context, *const RuntimeApi`), found {n} arguments"
                ),
            ))
        }
    };

    check_context_arg(args[0])?;
    if legacy {
        check_legacy_api_arg(args[1])?;
    }
    check_result_return(&input)?;

    let call = if legacy {
        quote! { #fn_name(&mut ctx, api) }
    } else {
        quote! { #fn_name(&mut ctx) }
    };

    let expanded = quote! {
        #original

        #[unsafe(no_mangle)]
        #vis extern "C" fn morrow_mod_init(
            api: *const ::morrow::RuntimeApi
        ) -> u32 {
            // Store the API for event-side free functions and logging macros.
            ::morrow::__internal::store_api(api, env!("CARGO_PKG_NAME"));
            let mut ctx = ::morrow::Context::from_api(api, env!("CARGO_PKG_NAME"));
            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| #call)) {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => 0,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    ::morrow::error!("Init failed: {:?}", e);
                    1
                }
                ::std::result::Result::Err(payload) => {
                    let msg = payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                        .unwrap_or("(unknown panic)");
                    ::morrow::error!("Init panicked: {}", msg);
                    1
                }
            }
        }
    };

    Ok(expanded.into())
}

/// Argument 0 must be `&Context` or `&mut Context`.
fn check_context_arg(arg: &FnArg) -> syn::Result<()> {
    let FnArg::Typed(PatType { ty, .. }) = arg else {
        return Err(syn::Error::new_spanned(
            arg,
            "#[morrow::mod_main] first argument must be `&mut Context`",
        ));
    };
    if !is_reference_to(ty, "Context") {
        return Err(syn::Error::new_spanned(
            ty,
            "#[morrow::mod_main] first argument must be `&mut Context`",
        ));
    }
    Ok(())
}

/// Legacy arg 1 must be `*const RuntimeApi`.
fn check_legacy_api_arg(arg: &FnArg) -> syn::Result<()> {
    let FnArg::Typed(PatType { ty, .. }) = arg else {
        return Err(syn::Error::new_spanned(
            arg,
            "#[morrow::mod_main] second argument must be `*const RuntimeApi`",
        ));
    };
    let Type::Ptr(ptr) = &**ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "#[morrow::mod_main] second argument must be `*const RuntimeApi`",
        ));
    };
    if !is_path_named(&ptr.elem, "RuntimeApi") {
        return Err(syn::Error::new_spanned(
            ty,
            "#[morrow::mod_main] second argument must be `*const RuntimeApi`",
        ));
    }
    Ok(())
}

/// Return type must be `Result<_, _>`.
fn check_result_return(input: &ItemFn) -> syn::Result<()> {
    let ReturnType::Type(_, ty) = &input.sig.output else {
        return Err(syn::Error::new_spanned(
            &input.sig.output,
            "#[morrow::mod_main] function must return `Result<(), MorrowError>`",
        ));
    };
    if !is_path_named(ty, "Result") {
        return Err(syn::Error::new_spanned(
            ty,
            "#[morrow::mod_main] function must return `Result<(), MorrowError>`",
        ));
    }
    Ok(())
}

fn is_reference_to(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Reference(TypeReference { elem, .. }) => is_path_named(elem, name),
        _ => false,
    }
}

fn is_path_named(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Path(TypePath { path, .. }) => path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == name),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// #[morrow::event(kind)]
// ---------------------------------------------------------------------------

/// Registers an event handler for the given event kind.
///
/// Supported kinds and handler signatures:
///
/// | kind | handler |
/// |------|---------|
/// | `tick` | `fn(u64)` |
/// | `server_start` | `fn()` |
/// | `server_stop` | `fn()` |
/// | `player_join`, `player_leave` | `fn(&str)` |
/// | `chat_message`, `block_break`, `block_place`, `player_death` | `fn(&str, &str)` |
///
/// The handler keeps a plain Rust signature; the macro generates the
/// `extern "C"` export the runtime discovers. Use at most one handler
/// per event kind per mod (a second one fails at link time with a
/// duplicate-symbol error).
///
/// # Example
///
/// ```ignore
/// #[morrow::event(player_join)]
/// fn on_join(player: &str) {
///     morrow::send_message(&format!("Welcome, {player}!"));
/// }
/// ```
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    let kind = parse_macro_input!(attr as Ident);
    let input = parse_macro_input!(item as ItemFn);
    match expand_event(&kind, input) {
        Ok(ts) => ts,
        Err(e) => e.to_compile_error().into(),
    }
}

/// Expected handler argument types for an event kind.
enum ArgSpec {
    Str, // &str
    U64, // u64
}

struct EventSpec {
    symbol: &'static str,
    args: &'static [ArgSpec],
}

fn event_spec(kind: &str) -> Option<EventSpec> {
    Some(EventSpec {
        symbol: match kind {
            "tick" => "morrow_mod_tick",
            "server_start" => "morrow_mod_server_start",
            "server_stop" => "morrow_mod_server_stop",
            "player_join" => "morrow_mod_player_join",
            "player_leave" => "morrow_mod_player_leave",
            "chat_message" => "morrow_mod_chat_message",
            "block_break" => "morrow_mod_block_break",
            "block_place" => "morrow_mod_block_place",
            "player_death" => "morrow_mod_player_death",
            _ => return None,
        },
        args: match kind {
            "tick" => &[ArgSpec::U64],
            "server_start" | "server_stop" => &[],
            "player_join" | "player_leave" => &[ArgSpec::Str],
            _ => &[ArgSpec::Str, ArgSpec::Str],
        },
    })
}

fn expand_event(kind: &Ident, input: ItemFn) -> syn::Result<TokenStream> {
    let spec = event_spec(&kind.to_string()).ok_or_else(|| {
        syn::Error::new_spanned(
            kind,
            format!(
                "unknown event kind '{kind}'; expected one of: \
                 tick, server_start, server_stop, player_join, player_leave, \
                 chat_message, block_break, block_place, player_death"
            ),
        )
    })?;

    // Validate signature
    if !input.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.sig.generics,
            format!("#[morrow::event({kind})] handler must not be generic"),
        ));
    }
    let args: Vec<&FnArg> = input.sig.inputs.iter().collect();
    if args.first().is_some_and(|a| matches!(a, FnArg::Receiver(_))) {
        return Err(syn::Error::new_spanned(
            &input.sig,
            format!("#[morrow::event({kind})] cannot be used on a method (no `self`)"),
        ));
    }
    if args.len() != spec.args.len() {
        return Err(syn::Error::new_spanned(
            &input.sig,
            format!(
                "#[morrow::event({kind})] expects {} argument(s), found {}",
                spec.args.len(),
                args.len()
            ),
        ));
    }
    for (i, (arg, want)) in args.iter().zip(spec.args).enumerate() {
        let FnArg::Typed(PatType { ty, .. }) = arg else {
            unreachable!("receiver checked above");
        };
        let ok = match want {
            ArgSpec::Str => is_reference_to(ty, "str"),
            ArgSpec::U64 => is_path_named(ty, "u64"),
        };
        if !ok {
            return Err(syn::Error::new_spanned(
                ty,
                format!(
                    "#[morrow::event({kind})] argument {} must be `{}`",
                    i + 1,
                    match want {
                        ArgSpec::Str => "&str",
                        ArgSpec::U64 => "u64",
                    }
                ),
            ));
        }
    }
    if !returns_unit(&input) {
        return Err(syn::Error::new_spanned(
            &input.sig.output,
            format!("#[morrow::event({kind})] handler must return `()`"),
        ));
    }

    let fn_name = &input.sig.ident;
    let original = &input;
    let export = Ident::new(spec.symbol, Span::call_site());

    let body = match spec.args {
        [] => quote! { #export() { #fn_name() } },
        [ArgSpec::U64] => quote! {
            #export(t: u64) { #fn_name(t) }
        },
        [ArgSpec::Str] => quote! {
            #export(a: *const u8, al: u32) {
                #fn_name(::morrow::read_str(a, al));
            }
        },
        _ => quote! {
            #export(a: *const u8, al: u32, b: *const u8, bl: u32) {
                #fn_name(::morrow::read_str(a, al), ::morrow::read_str(b, bl));
            }
        },
    };

    let expanded = quote! {
        #original

        #[unsafe(no_mangle)]
        pub extern "C" fn #body
    };

    Ok(expanded.into())
}

fn returns_unit(input: &ItemFn) -> bool {
    match &input.sig.output {
        ReturnType::Default => true,
        ReturnType::Type(_, ty) => match &**ty {
            Type::Tuple(t) => t.elems.is_empty(),
            _ => false,
        },
    }
}
