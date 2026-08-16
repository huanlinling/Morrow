//! Hello Morrow — full API demo. Zero unsafe, zero hand-written FFI.

use morrow::prelude::*;

#[morrow::mod_main]
fn init(ctx: &mut Context) -> Result<(), MorrowError> {
    match ctx.config() {
        Some(cfg) => morrow::info!("config.toml: {} bytes", cfg.len()),
        None => morrow::info!("no config.toml packaged"),
    }
    for cap in ["commands", "host_api", "config"] {
        let v = ctx.request_capability(cap);
        if v > 0 {
            morrow::info!("Cap {}: v{}", cap, v);
        }
    }
    ctx.register_command("morrow", morrow_cmd);
    Ok(())
}

fn morrow_cmd(_args: &str) {
    let players = morrow::player_list();
    let t = morrow::world_time();
    morrow::send_message(&format!("Players: {}. Time: {}.", players.join(", "), t));
}

#[morrow::event(server_start)]
fn on_server_start() {
    morrow::info!("Ready!");
}

#[morrow::event(server_stop)]
fn on_server_stop() {
    morrow::info!("Bye!");
}

#[morrow::event(tick)]
fn on_tick(t: u64) {
    if t % 200 == 0 {
        morrow::info!("tick {}", t);
    }
}

#[morrow::event(player_join)]
fn on_join(player: &str) {
    morrow::info!("+ {}", player);
}

#[morrow::event(player_leave)]
fn on_leave(player: &str) {
    morrow::info!("- {}", player);
}

#[morrow::event(chat_message)]
fn on_chat(player: &str, msg: &str) {
    morrow::info!("<{}> {}", player, msg);
}

#[morrow::event(block_break)]
fn on_block_break(player: &str, block: &str) {
    morrow::info!("{} broke {}", player, block);
}

#[morrow::event(block_place)]
fn on_block_place(player: &str, block: &str) {
    morrow::info!("{} placed {}", player, block);
}

#[morrow::event(player_death)]
fn on_death(player: &str, _cause: &str) {
    morrow::info!("{} died", player);
}
