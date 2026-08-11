//! Global logger — always prints to stderr (captured by Minecraft).
//! Also forwards to Java Host API when available.

use log::{Level, Log, Metadata, Record};

struct MorrowLogger;

impl Log for MorrowLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool { true }

    fn log(&self, record: &Record) {
        let level_code = match record.level() {
            Level::Error => 3,
            Level::Warn => 2,
            Level::Info => 1,
            _ => 0,
        };
        let msg = format!("{}", record.args());

        // Always print to stderr (Minecraft captures this as [STDOUT])
        eprintln!("{}", msg);

        // Also forward to Java via HostApi if available
        if let Ok(apis) = crate::HOST_APIS.lock() {
            if let Some(api) = apis.values().next() {
                api.log_message(level_code, &msg);
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: MorrowLogger = MorrowLogger;

pub(crate) fn init() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(log::LevelFilter::Debug))
        .ok();
}
