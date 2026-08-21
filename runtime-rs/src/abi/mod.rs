//! ABI contract with the Java host. See docs/02-abi-design.md.
//!
//! Changes here require a synchronized change in PanamaBridge.java.

pub mod handles;

/// Version of the ABI this runtime implements.
///
/// Encoding: major in high 16 bits, minor in low 16 bits.
/// Same major version = compatible.
pub const ABI_VERSION: u32 = 0x0001_0000; // v1.0

/// Check whether `requested` ABI version is compatible with `actual`.
pub const fn is_abi_compatible(requested: u32, actual: u32) -> bool {
    requested >> 16 == actual >> 16
}

// ---------------------------------------------------------------------------
// Error codes — returned by all lifecycle exports as u32
// ---------------------------------------------------------------------------

/// Operation succeeded.
pub const RESULT_OK: u32 = 0;
/// Unknown / internal error.
pub const RESULT_ERR_UNKNOWN: u32 = 1;
/// The given handle is invalid (0, or not found in the registry).
pub const RESULT_ERR_INVALID_HANDLE: u32 = 3;
/// Illegal state transition (e.g. double shutdown).
pub const RESULT_ERR_WRONG_STATE: u32 = 4;
/// A Rust panic was caught at the FFI boundary.
pub const RESULT_ERR_PANIC: u32 = 0xFF;
