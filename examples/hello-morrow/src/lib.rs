//! Hello Morrow — demo mod showing the full API.
//!
//! Features: commands, player events, block events, world queries, chat.

use morrow::prelude::*;

static mut API: Option<RuntimeApi> = None;

#[morrow::mod_main]
fn init(_ctx: &mut Context, api: *const RuntimeApi) -> Result<(), MorrowError> {
    let a = unsafe { api.read() };

    // Check capabilities
    let caps: &[&[u8]] = &[b"commands", b"host_api", b"config", b"magic"];
    for &cap in caps {
        let cap_name = std::str::from_utf8(cap).unwrap();
        let ver = unsafe { (a.request_capability)(0, cap.as_ptr(), cap.len() as u32) };
        if ver > 0 {
            morrow::info!("Capability {}: v{}", cap_name, ver);
        }
    }

    // Read config
    let mut cbuf = [0u8; 512];

    unsafe {
        (a.register_command)(0, b"morrow".as_ptr(), 6, morrow_cmd);
        (a.register_command)(0, b"day".as_ptr(), 3, day_cmd);
        API = Some(a);
    }
    Ok(())
}

// ─── Commands ──────────────────────────────────

unsafe extern "C" fn morrow_cmd(args_ptr: *const u8, args_len: u32) {
    let args = unsafe {
        let bytes = std::slice::from_raw_parts(args_ptr, args_len as usize);
        String::from_utf8_lossy(bytes)
    };
    if let Some(ref api) = unsafe { API.as_ref() } {
        let mut buf = [0u8; 256];
        let n = unsafe { (api.get_player_list)(0, buf.as_mut_ptr(), buf.len() as u32) };
        let list = std::str::from_utf8(&buf[..n as usize]).unwrap_or("?");
        let time = unsafe { (api.get_world_time)(0) };
        let msg = format!("Players: {}. Time: {}. Args: {args}", list, time);
        let b = msg.as_bytes();
        unsafe { (api.send_message)(0, b.as_ptr(), b.len() as u32); }
    }
}

unsafe extern "C" fn day_cmd(_: *const u8, _: u32) {
    if let Some(ref api) = unsafe { API.as_ref() } {
        let cmd = b"time set day";
        unsafe { (api.execute_command)(0, cmd.as_ptr(), cmd.len() as u32); }
        let msg = b"Time set to day!";
        unsafe { (api.send_message)(0, msg.as_ptr(), msg.len() as u32); }
    }
}

// ─── Lifecycle ─────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_server_start() {
    morrow::info!("Ready! Try /morrow or /day");
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_server_stop() {
    morrow::info!("Goodbye!");
}

// ─── Tick ──────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_tick(tick: u64) {
    if tick == 200 {
        morrow::info!("Server has been running for 10 seconds");
    }
}

// ─── Player events ────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_player_join(name_ptr: *const u8, name_len: u32) {
    let name = unsafe {
        let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
        String::from_utf8_lossy(bytes)
    };
    morrow::info!("+ {}", name);
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_player_leave(name_ptr: *const u8, name_len: u32) {
    let name = unsafe {
        let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
        String::from_utf8_lossy(bytes)
    };
    morrow::info!("- {}", name);
}

// ─── Chat ──────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_chat_message(
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
    morrow::info!("<{}> {}", player, msg);
}

// ─── Block events ──────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_block_break(
    player_ptr: *const u8, player_len: u32,
    block_ptr: *const u8, block_len: u32,
) {
    let player = unsafe {
        let bytes = std::slice::from_raw_parts(player_ptr, player_len as usize);
        String::from_utf8_lossy(bytes)
    };
    let block = unsafe {
        let bytes = std::slice::from_raw_parts(block_ptr, block_len as usize);
        String::from_utf8_lossy(bytes)
    };
    morrow::info!("{} broke {}", player, block);
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_block_place(
    player_ptr: *const u8, player_len: u32,
    block_ptr: *const u8, block_len: u32,
) {
    let player = unsafe {
        let bytes = std::slice::from_raw_parts(player_ptr, player_len as usize);
        String::from_utf8_lossy(bytes)
    };
    let block = unsafe {
        let bytes = std::slice::from_raw_parts(block_ptr, block_len as usize);
        String::from_utf8_lossy(bytes)
    };
    morrow::info!("{} placed {}", player, block);
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_player_death(
    player_ptr: *const u8, player_len: u32,
    msg_ptr: *const u8, msg_len: u32,
) {
    let player = unsafe {
        let bytes = std::slice::from_raw_parts(player_ptr, player_len as usize);
        String::from_utf8_lossy(bytes)
    };
    morrow::info!("{} died", player);
}
