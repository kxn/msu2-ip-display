pub fn info(message: &str) {
    eprintln!("INFO miniboard-ipd: {message}");
}

pub fn warn(message: &str) {
    eprintln!("WARN miniboard-ipd: {message}");
}

pub fn debug(message: &str) {
    if std::env::var_os("MINIBOARD_IPD_DEBUG").is_some() {
        eprintln!("DEBUG miniboard-ipd: {message}");
    }
}
