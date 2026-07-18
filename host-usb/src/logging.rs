use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enable_debug() {
    DEBUG_ENABLED.store(true, Ordering::Relaxed);
}

pub fn info(message: &str) {
    eprintln!("INFO miniboard-ipd: {message}");
}

pub fn warn(message: &str) {
    eprintln!("WARN miniboard-ipd: {message}");
}

pub fn debug(message: &str) {
    if DEBUG_ENABLED.load(Ordering::Relaxed) || std::env::var_os("MINIBOARD_IPD_DEBUG").is_some() {
        eprintln!("DEBUG miniboard-ipd: {message}");
    }
}
