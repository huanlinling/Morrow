//! Hello Ferrum — demo mod showing the full API surface.
//!
//! Features: init, lifecycle, tick, player events, commands, chat.

use ferrum::prelude::*;

static mut API: Option<RuntimeApi> = None;

/// Mod entry point — registers a command and stores the API.
#[ferrum::mod_main]
fn init(_ctx: &mut Context, api: *const RuntimeApi) -> Result<(), FerrumError> {
    ferrum::info!("Hello from Rust!");
    let api_ref = unsafe { api.read() };
    // Register a command
    let api_ptr = api;
    unsafe {
        let name = b"ferrum";
        (api_ref.register_command)(0, name.as_ptr(), name.len() as u32, ferrum_command_handler);
        API = Some(api_ref);
    }
    Ok(())
}

// ─── Command handler ──────────────────────────

unsafe extern "C" fn ferrum_command_handler(args_ptr: *const u8, args_len: u32) {
    let args = unsafe {
        let bytes = std::slice::from_raw_parts(args_ptr, args_len as usize);
        String::from_utf8_lossy(bytes)
    };
    let msg = format!("Ferrum command executed! Args: {args}");
    let msg_bytes = msg.as_bytes();
    if let Some(ref api) = unsafe { API.as_ref() } {
        unsafe { (api.send_message)(0, msg_bytes.as_ptr(), msg_bytes.len() as u32); }
    }
}

// ─── Lifecycle ─────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_server_start() {
    ferrum::info!("Server started! Try /ferrum hello");
}

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_server_stop() {
    ferrum::info!("Server stopping...");
}

// ─── Tick ──────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_tick(tick: u64) {
    if tick % 20 == 0 {
        let players = unsafe {
            API.as_ref().map(|api| (api.get_player_count)(0)).unwrap_or(-1)
        };
        if tick % 200 == 0 { // every 10 seconds
            ferrum::info!("Tick {}: {} players online", tick, players);
        }
    }
}

// ─── Player events ────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_player_join(name_ptr: *const u8, name_len: u32) {
    let name = unsafe {
        let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
        String::from_utf8_lossy(bytes)
    };
    ferrum::info!("Player joined: {}", name);
}

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_player_leave(name_ptr: *const u8, name_len: u32) {
    let name = unsafe {
        let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
        String::from_utf8_lossy(bytes)
    };
    ferrum::info!("Player left: {}", name);
}

// ─── Chat ──────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn ferrum_mod_chat_message(
    player_ptr: *const u8, player_len: u32,
    msg_ptr: *const u8, msg_len: u32,
) {
    let player = unsafe {
        let bytes = std::slice::from_raw_parts(player_ptr, player_len as usize);
        String::from_utf8_lossy(bytes)
    };
    let msg = unsafe {
        let bytes = std::slice::from_raw_parts(msg_ptr, msg_len as usize);
        String::from_utf8_lossy(bytes)
    };
    if msg.contains("ferrum") {
        let reply = format!("{} mentioned ferrum!", player);
        let bytes = reply.as_bytes();
        if let Some(ref api) = unsafe { API.as_ref() } {
            unsafe { (api.send_message)(0, bytes.as_ptr(), bytes.len() as u32); }
        }
    }
}
