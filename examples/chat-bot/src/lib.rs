//! Chat Bot — responds to chat messages.
//!
//! Demonstrates: chat events, send_message, commands.

use morrow::prelude::*;

static mut API: Option<RuntimeApi> = None;

#[morrow::mod_main]
fn init(_ctx: &mut Context, api: *const RuntimeApi) -> Result<(), MorrowError> {
    let a = unsafe { api.read() };
    unsafe {
        (a.register_command)(0, b"ping".as_ptr(), 4, ping_cmd);
        API = Some(a);
    }
    morrow::info!("Chat bot online! Say hi or /ping");
    Ok(())
}

unsafe extern "C" fn ping_cmd(_: *const u8, _: u32) {
    if let Some(ref api) = unsafe { API.as_ref() } {
        let msg = b"Pong!";
        unsafe { (api.send_message)(0, msg.as_ptr(), msg.len() as u32); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn morrow_mod_player_join(name_ptr: *const u8, name_len: u32) {
    let name = unsafe {
        let bytes = std::slice::from_raw_parts(name_ptr, name_len as usize);
        String::from_utf8_lossy(bytes)
    };
    if let Some(ref api) = unsafe { API.as_ref() } {
        let msg = format!("Welcome, {}!", name);
        let b = msg.as_bytes();
        unsafe { (api.send_message)(0, b.as_ptr(), b.len() as u32); }
    }
}

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
        String::from_utf8_lossy(bytes).to_lowercase()
    };

    if let Some(ref api) = unsafe { API.as_ref() } {
        let reply: Option<&str> = if msg.contains("hello") || msg.contains("hi") {
            Some("Hello there!")
        } else if msg.contains("morrow") {
            Some("Morrow is awesome! 🦀")
        } else if msg.contains("time") {
            let t = unsafe { (api.get_world_time)(0) };
            return send(api, format!("World time: {} ticks", t));
        } else {
            None
        };

        if let Some(r) = reply {
            send(api, format!("{}: {}", player, r));
        }
    }
}

fn send(api: &RuntimeApi, msg: String) {
    let b = msg.as_bytes();
    unsafe { (api.send_message)(0, b.as_ptr(), b.len() as u32); }
}
