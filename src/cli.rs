//! # Command Line Interface

pub struct Args {
    pub debug: bool,
}

pub fn parse() -> Args {
    let mut debug = std::env::var("X11IDLE_DEBUG").is_ok();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-d" | "--debug" => debug = true,
            "-v" | "--version" => {
                println!("x11idle {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-h" | "--help" => {
                println!("x11idle — X11 idle daemon with D-Bus integration\n");
                println!("Usage: x11idle [OPTIONS]\n");
                println!("Options:");
                println!("  -d, --debug    Enable verbose debug output (or set X11IDLE_DEBUG)");
                println!("  -h, --help     Show this help message");
                println!("  -v, --version  Show version");
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown option: {}", other);
                std::process::exit(1);
            }
        }
    }

    Args { debug }
}
