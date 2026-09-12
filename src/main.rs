// Entry point — owned by the coordinator. Worker 4 owns app.rs.
#![windows_subsystem = "windows"]

fn main() {
    let mode = redsamurai_config::tray::StartupMode::from_args(std::env::args());
    let result = if mode.is_tray() {
        redsamurai_config::tray::run_tray_mode().map_err(|error| error.to_string())
    } else {
        redsamurai_config::app::run().map_err(|error| error.to_string())
    };

    if let Err(error) = result {
        eprintln!("RED SAMURAI: {error}");
        std::process::exit(1);
    }
}
