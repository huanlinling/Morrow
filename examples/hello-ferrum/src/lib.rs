//! Hello Ferrum — the simplest possible mod.
//!
//! Demonstrates the full lifecycle: init, server start/stop, tick,
//! and upcalls (querying player count from Java).

use ferrum::prelude::*;

/// Mod entry point — receives a pointer to the runtime's function table.
#[ferrum::mod_main]
fn init(_ctx: &mut Context, api: *const RuntimeApi) -> Result<(), FerrumError> {
    ferrum::info!("Hello from Rust!");
    // Store the API for later use
    unsafe { API = Some(api.read()); }
    Ok(())
}

static mut API: Option<RuntimeApi> = None;

/// Called when the server finishes starting.
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_server_start() {
    ferrum::info!("Server started!");
}

/// Called when the server begins stopping.
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_server_stop() {
    ferrum::info!("Server stopping...");
}

/// Called every game tick (20 TPS).
#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_tick(tick: u64) {
    if tick % 20 == 0 {
        let players = unsafe {
            API.as_ref()
                .map(|api| (api.get_player_count)(0))
                .unwrap_or(-1)
        };
        ferrum::info!("Tick {}: second {} passed! Players online: {}",
            tick, tick / 20, players);
    }
}
