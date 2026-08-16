//! motd — reads its welcome message from config.toml and greets every
//! joining player. Demonstrates config access with zero unsafe.

use morrow::prelude::*;

/// Parse `message = "..."` out of config.toml (raw TOML text).
fn motd() -> String {
    morrow::config()
        .and_then(|cfg| {
            cfg.lines().find_map(|line| {
                line.trim()
                    .split_once('=')
                    .filter(|(k, _)| k.trim() == "message")
                    .map(|(_, v)| v.trim().trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "Welcome to the server!".to_string())
}

#[morrow::mod_main]
fn init(ctx: &mut Context) -> Result<(), MorrowError> {
    ctx.register_command("motd", motd_cmd);
    morrow::info!("MOTD: {}", motd());
    Ok(())
}

fn motd_cmd(_args: &str) {
    morrow::send_message(&motd());
}

#[morrow::event(player_join)]
fn on_join(player: &str) {
    morrow::send_message(&format!("{} — welcome, {}!", motd(), player));
}
