//! Diagnostic console tool for RED SAMURAI Button 16 & Raw Input verification.
//!
//! This tool provides real-time diagnostic logs required to verify:
//! - expected_path vs observed Raw Input device path
//! - Path normalization before and after
//! - is_target_device evaluation
//! - virtual_key, scan_code, make/break edge
//! - Dispatch branch: target_device branch vs non-target branch
//! - Low-level keyboard hook suppression & replay behavior

use std::time::{Duration, Instant};

#[cfg(windows)]
use redsamurai_config::resident_platform::{
    is_resident_input_interface, normalize_raw_input_device_path,
    open_resident_input_device_for_service, KeyboardEvent, ResidentInputCandidate,
    ResidentPlatform, RED_SAMURAI_PRODUCT_ID, RED_SAMURAI_VENDOR_ID,
    RESIDENT_INPUT_REPORT_LENGTH,
};

struct DiagnosticConfig {
    vk: u16,
    scan: u32,
    timeout: Option<Duration>,
    dry_run: bool,
    send_action: bool,
}

impl Default for DiagnosticConfig {
    fn default() -> Self {
        Self {
            // Default: '0' key (VK 0x30, scan code 0x0B) which RED SAMURAI sends for Button 16
            vk: 0x30,
            scan: 0x0B,
            timeout: None,
            dry_run: false,
            send_action: false,
        }
    }
}

fn parse_args() -> Result<DiagnosticConfig, String> {
    let mut config = DiagnosticConfig::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--vk" => {
                let val_str = args.next().ok_or("Missing value for --vk")?;
                let val = if val_str.starts_with("0x") || val_str.starts_with("0X") {
                    u16::from_str_radix(&val_str[2..], 16)
                } else {
                    val_str.parse::<u16>()
                }
                .map_err(|e| format!("Invalid --vk value '{val_str}': {e}"))?;
                config.vk = val;
            }
            "--scan" => {
                let val_str = args.next().ok_or("Missing value for --scan")?;
                let val = if val_str.starts_with("0x") || val_str.starts_with("0X") {
                    u32::from_str_radix(&val_str[2..], 16)
                } else {
                    val_str.parse::<u32>()
                }
                .map_err(|e| format!("Invalid --scan value '{val_str}': {e}"))?;
                config.scan = val;
            }
            "--timeout" => {
                let sec_str = args.next().ok_or("Missing value for --timeout")?;
                let secs = sec_str
                    .parse::<u64>()
                    .map_err(|e| format!("Invalid --timeout value '{sec_str}': {e}"))?;
                config.timeout = Some(Duration::from_secs(secs));
            }
            "--dry-run" => {
                config.dry_run = true;
            }
            "--send-action" => {
                config.send_action = true;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                return Err(format!("Unknown argument: {other}. Use --help for usage."));
            }
        }
    }

    Ok(config)
}

fn print_help() {
    println!(
        "RED SAMURAI Button 16 Hardware & Raw Input Diagnostic Tool\n\n\
        Usage: redsamurai-diagnose [OPTIONS]\n\n\
        Options:\n\
          --vk <hex/dec>      Virtual key code to suppress (default: 0x30 for '0')\n\
          --scan <hex/dec>    Scan code to suppress (default: 0x0B for '0')\n\
          --timeout <secs>    Automatically exit after the specified seconds\n\
          --send-action       Emit Ctrl+Alt+Shift+P SendInput on Button 16 (for screenshot test)\n\
          --dry-run           List HID devices and candidates without starting Raw Input\n\
          -h, --help          Print this help message\n"
    );
}

fn main() {
    // Enable diagnostic console logging from resident_platform
    std::env::set_var("REDSAMURAI_CONSOLE_LOG", "1");

    println!("========================================================================");
    println!("       RED SAMURAI Button 16 & Raw Input Diagnostic Tool               ");
    println!("========================================================================");

    let config = match parse_args() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    };

    #[cfg(not(windows))]
    {
        eprintln!("Error: This diagnostic tool requires Windows platform.");
        std::process::exit(1);
    }

    #[cfg(windows)]
    run_windows_diagnose(config);
}

#[cfg(windows)]
fn run_windows_diagnose(config: DiagnosticConfig) {
    println!(
        "Suppression Target: VK=0x{:02X}, ScanCode=0x{:02X}",
        config.vk, config.scan
    );
    if let Some(t) = config.timeout {
        println!("Timeout: {}s", t.as_secs());
    }
    println!();

    println!("Scanning connected HID devices...");
    let api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            eprintln!("Failed to initialize hidapi: {e}");
            std::process::exit(1);
        }
    };

    let mut redsamurai_candidates = Vec::new();
    let mut matching_target = None;

    for info in api.device_list() {
        if info.vendor_id() == RED_SAMURAI_VENDOR_ID && info.product_id() == RED_SAMURAI_PRODUCT_ID
        {
            let raw_path = info.path().to_string_lossy().into_owned();
            let norm_path = normalize_raw_input_device_path(&raw_path);
            let candidate = ResidentInputCandidate::new(
                raw_path.clone(),
                info.vendor_id(),
                info.product_id(),
                info.usage_page(),
                info.usage(),
                info.interface_number(),
                RESIDENT_INPUT_REPORT_LENGTH,
            );
            let is_match = is_resident_input_interface(&candidate);
            if is_match && matching_target.is_none() {
                matching_target = Some((raw_path.clone(), norm_path.clone()));
            }
            redsamurai_candidates.push((candidate, raw_path, norm_path, is_match));
        }
    }

    println!(
        "Found {} RED SAMURAI HID endpoint(s):",
        redsamurai_candidates.len()
    );
    for (i, (candidate, raw_path, norm_path, is_match)) in
        redsamurai_candidates.iter().enumerate()
    {
        let status = if *is_match {
            "[TARGET MATCH]"
        } else {
            "[NON-TARGET]"
        };
        println!("  [{i}] {status}");
        println!("      Raw Path    : {raw_path}");
        println!("      Norm Path   : {norm_path}");
        println!(
            "      Interface   : {}, UsagePage: 0x{:04X}, Usage: 0x{:04X}",
            candidate.interface_number, candidate.usage_page, candidate.usage
        );
    }
    println!();

    let (_expected_raw, _expected_norm) = match matching_target {
        Some(target) => {
            println!("Selected Target Device (expected_path):");
            println!("  Expected Raw Path : {}", target.0);
            println!("  Expected Norm Path: {}", target.1);
            println!();
            target
        }
        None => {
            eprintln!("WARNING: No matching RED SAMURAI resident input collection found!");
            eprintln!("Make sure the mouse is connected (VID: 0x04D9, PID: 0xFC55).");
            if config.dry_run {
                println!("Dry run complete.");
                return;
            }
            std::process::exit(1);
        }
    };

    if config.dry_run {
        println!("Dry run complete.");
        return;
    }

    println!("Starting resident platform service listener...");
    let initial_suppressed = [(config.vk, config.scan)];
    let mut device = match open_resident_input_device_for_service(&initial_suppressed) {
        Ok(dev) => dev,
        Err(e) => {
            eprintln!("Failed to open resident input device: {e:?}");
            std::process::exit(1);
        }
    };

    println!("========================================================================");
    println!("Listening for Raw Input and Button events.");
    println!(">> PLEASE PRESS BUTTON 16 (OR SIDE BUTTONS) ON THE MOUSE! <<");
    println!("All events will be displayed below with detailed diagnostic traces.");
    println!("Press Ctrl+C to exit.");
    println!("========================================================================");
    println!();

    let start_time = Instant::now();
    let mut event_count = 0usize;

    loop {
        if let Some(timeout) = config.timeout {
            if start_time.elapsed() >= timeout {
                println!("\nReached configured timeout of {}s. Exiting.", timeout.as_secs());
                break;
            }
        }

        // Read button transitions with 50ms polling timeout
        match device.read_transitions(50) {
            Ok(Some(transitions)) => {
                for transition in transitions {
                    event_count += 1;
                    let action = if transition.is_pressed() {
                        "PRESSED"
                    } else {
                        "RELEASED"
                    };
                    println!(
                        "[DIAGNOSE-RESULT #{event_count}] Target Device Button Transition: Usage=0x{:02X} (edge: {})",
                        transition.usage(),
                        action
                    );
                    if config.send_action && transition.usage() == 0x27 {
                        if transition.is_pressed() {
                            println!("[DIAGNOSE-ACTION] Emitting Ctrl+Alt+Shift+P down via SendInput...");
                            let _ = ResidentPlatform::send_keyboard(KeyboardEvent::key_down(0x11)); // VK_CONTROL
                            let _ = ResidentPlatform::send_keyboard(KeyboardEvent::key_down(0x12)); // VK_MENU (Alt)
                            let _ = ResidentPlatform::send_keyboard(KeyboardEvent::key_down(0x10)); // VK_SHIFT
                            let _ = ResidentPlatform::send_keyboard(KeyboardEvent::key_down(0x50)); // VK_P
                        } else {
                            println!("[DIAGNOSE-ACTION] Emitting Ctrl+Alt+Shift+P up via SendInput...");
                            let _ = ResidentPlatform::send_keyboard(KeyboardEvent::key_up(0x50));
                            let _ = ResidentPlatform::send_keyboard(KeyboardEvent::key_up(0x10));
                            let _ = ResidentPlatform::send_keyboard(KeyboardEvent::key_up(0x12));
                            let _ = ResidentPlatform::send_keyboard(KeyboardEvent::key_up(0x11));
                        }
                    }
                }
            }
            Ok(None) => {
                // Timeout on read, continue
            }
            Err(e) => {
                eprintln!("[DIAGNOSE-ERROR] read_transitions failed: {e:?}");
                break;
            }
        }
    }

    println!("Diagnostic session completed. Total events received: {event_count}");
}
