//! Panic boundary for FFI exports.
//!
//! Every `extern "C"` function must be wrapped so Rust panics never
//! unwind into C/JVM stack frames (which is undefined behavior).
//!
//! See docs/02-abi-design.md § "绝不跨 FFI unwind".

use std::panic::{self, AssertUnwindSafe};

/// Run `f` inside [`catch_unwind`], returning `default` if a panic occurs.
///
/// The panic payload is logged to stderr. This is the single choke-point
/// for all FFI panic safety — every `extern "C"` export delegates here.
pub fn ffi_boundary<F, T>(default: T, f: F) -> T
where
    F: FnOnce() -> T,
{
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("<non-string panic payload>");
            eprintln!("[Morrow] panic caught at FFI boundary: {msg}");
            default
        }
    }
}
