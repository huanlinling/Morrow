//! Hello Ferrum — the simplest possible mod.
//!
//! Uses the Ferrum SDK. This is the idiomatic way to write a Ferrum mod.

use ferrum::prelude::*;

/// Mod entry point — annotated with `#[ferrum::mod_main]`.
///
/// The macro generates `ferrum_mod_init()` automatically.
/// The runtime calls this during mod loading.
#[ferrum::mod_main]
fn init(_ctx: &mut Context) -> Result<(), FerrumError> {
    ferrum::info!("Hello from Rust!");
    Ok(())
}

/// Optional tick callback — manually exported for the runtime to discover.
///
/// The runtime automatically discovers `ferrum_mod_tick` during mod loading
/// and calls it every game tick (20 TPS).
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_tick(tick: u64) {
    if tick % 20 == 0 {
        ferrum::info!("Tick {}: second {} passed!", tick, tick / 20);
    }
}
