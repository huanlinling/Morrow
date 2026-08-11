//! Proc macros for the Ferrum modding SDK.
//!
//! `#[ferrum::mod_main]` — marks a function as the mod entry point.

use proc_macro::TokenStream;
use quote::quote;

/// Marks a function as the entry point for a Ferrum mod.
///
/// The function must have signature:
/// `fn(&mut Context, *const RuntimeApi) -> Result<(), FerrumError>`
///
/// The macro generates `ferrum_mod_init(api: *const RuntimeApi) -> u32`
/// which the runtime calls with a pointer to its function table.
///
/// # Example
///
/// ```ignore
/// use ferrum::prelude::*;
///
/// #[ferrum::mod_main]
/// fn init(ctx: &mut Context, api: *const RuntimeApi) -> Result<(), FerrumError> {
///     ferrum::info!("Hello from my mod!");
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn mod_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input.sig.ident;
    let vis = &input.vis;

    let original = &input;

    let expanded = quote! {
        #original

        #[unsafe(no_mangle)]
        #vis extern "C" fn ferrum_mod_init(
            api: *const ::ferrum::RuntimeApi
        ) -> u32 {
            let mut ctx = ::ferrum::Context::new();
            match #fn_name(&mut ctx, api) {
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
