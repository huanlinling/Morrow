//! Chat Bot — responds to chat and joins. Zero unsafe, zero hand-written FFI.

use morrow::prelude::*;

#[morrow::mod_main]
fn init(ctx: &mut Context) -> Result<(), MorrowError> {
    ctx.register_command("ping", ping)?;
    morrow::info!("Chat bot online! Say hi or /ping");
    Ok(())
}

fn ping(_args: &str) {
    morrow::send_message("Pong!");
}

#[morrow::event(player_join)]
fn on_join(player: &str) {
    morrow::send_message(&format!("Welcome, {player}!"));
}

#[morrow::event(chat_message)]
fn on_chat(player: &str, msg: &str) {
    if msg.contains("hello") || msg.contains("hi") {
        morrow::send_message(&format!("{player}: Hello!"));
    } else if msg.contains("time") {
        let t = morrow::world_time();
        morrow::send_message(&format!("Time: {t} ticks"));
    }
}
