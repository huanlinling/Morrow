//! Proc macro for `#[ferrum::mod_main]`.
//!
//! This macro transforms a mod entry function into the `extern "C"` exports
//! that the Ferrum runtime expects: `ferrum_mod_init` and optionally
//! `ferrum_mod_tick`.

use proc_macro::TokenStream;
use quote::quote;

/// Marks a function as the entry point for a Ferrum mod.
///
/// The function must have signature `fn(&mut Context) -> Result<(), FerrumError>`.
///
/// The macro generates:
/// - `ferrum_mod_init()` — called by the runtime on load
/// - `ferrum_mod_tick(tick: u64)` — optional, if the user defines `on_tick`
///
/// # Example
///
/// ```ignore
/// use ferrum::prelude::*;
///
/// #[ferrum::mod_main]
/// fn init(ctx: &mut Context) -> Result<(), FerrumError> {
///     ferrum::info!("Hello from my mod!");
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn mod_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input.sig.ident;
    let _fn_body = &input.block;
    let vis = &input.vis;

    // Keep the original function
    let original = &input;

    let expanded = quote! {
        // Keep the user's function as-is
        #original

        // Generate the ferrum_mod_init entry point
        #[unsafe(no_mangle)]
        #vis extern "C" fn ferrum_mod_init() -> u32 {
            // Initialize logging with the crate name
            let mut ctx = ::ferrum::Context::new();
            match #fn_name(&mut ctx) {
                Ok(()) => 0,
                Err(e) => {
                    ::ferrum::error!("Init failed: {:?}", e);
                    1
                }
            }
        }
    };

    TokenStream::from(expanded)
}
