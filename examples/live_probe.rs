//! Explicit live boundary probe for the Windows HID and SendInput seams.
//!
//! The probe is opt-in and intentionally narrow: it enumerates the exact
//! configuration and resident collections, optionally reads one bounded
//! resident transition, or monitors the actual Raw Input path for a short,
//! explicitly bounded window.  It can also send a zero-distance or one-pixel
//! mouse-move record when that action is explicitly selected.
//! The default probe never sends a configuration feature report. The separate
//! `--apply-125hz --confirm-hid-apply` flags send only the evidence-backed
//! ordered 125 Hz sequence after an exact collection check; adding
//! `--observe-polling-readback` performs one bounded, read-only GET_REPORT
//! observation after that sequence. `--observe-dpi-selection-readback` is a
//! standalone context probe; a selected-DPI value is emitted only when the
//! full 40-byte post-Apply/reconnect response is present. The probe never
//! presses a key or mouse button. `--apply-dpi-profile 1|2
//! --confirm-hid-apply` is the bounded current-Rust selected-DPI proof: it
//! sends the complete authorized sequence, waits for a physical reconnect,
//! then performs one fresh-handle Report-03 readback.

use redsamurai_config::device_apply::ApplyOutcome;
use redsamurai_config::device_protocol::{DpiSelectionReadback, PollingRateReadback};
use redsamurai_config::resident_platform::ButtonTransition;
use std::env;

/// Maximum duration accepted by `--read-resident-for-ms`.
///
/// The monitor is intentionally finite even when a caller supplies a very
/// large value.  Thirty seconds is long enough for a manual edge check while
/// ensuring an accidentally unattended probe cannot remain resident.
pub const MAX_RESIDENT_READ_FOR_MS: u64 = 30_000;

const RESIDENT_READ_ONCE_TIMEOUT_MS: i32 = 250;

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

/// Parse the optional bounded resident monitor duration.
///
/// The duration mode is deliberately a stricter action boundary than the
/// legacy one-shot `--read-resident` probe: it requires both live
/// confirmation and the resident-read flag, rejects zero/invalid values, and
/// cannot be combined with configuration Apply or SendInput actions.  Values
/// above [`MAX_RESIDENT_READ_FOR_MS`] are capped rather than allowed to create
/// an unbounded monitor.
pub fn parse_resident_read_for_ms(args: &[String]) -> Result<Option<u64>, String> {
    let occurrences: Vec<usize> = args
        .iter()
        .enumerate()
        .filter_map(|(index, arg)| (arg == "--read-resident-for-ms").then_some(index))
        .collect();
    let Some(&index) = occurrences.first() else {
        return Ok(None);
    };
    if occurrences.len() > 1 {
        return Err("refusing resident monitor: pass --read-resident-for-ms only once".to_owned());
    }
    if !has_flag(args, "--confirm-live") {
        return Err(
            "refusing resident monitor: --read-resident-for-ms requires --confirm-live".to_owned(),
        );
    }
    if !has_flag(args, "--read-resident") {
        return Err(
            "refusing resident monitor: --read-resident-for-ms requires --read-resident".to_owned(),
        );
    }
    if has_flag(args, "--apply-125hz")
        || has_flag(args, "--sendinput-zero-move")
        || has_flag(args, "--sendinput-one-pixel")
    {
        return Err(
            "refusing resident monitor: --read-resident-for-ms cannot be combined with Apply or SendInput actions"
                .to_owned(),
        );
    }

    let value = args
        .get(index + 1)
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| {
            "refusing resident monitor: --read-resident-for-ms requires a positive millisecond value"
                .to_owned()
        })?;
    let requested = value.parse::<u64>().map_err(|_| {
        format!("refusing resident monitor: invalid --read-resident-for-ms value {value:?}")
    })?;
    if requested == 0 {
        return Err(
            "refusing resident monitor: --read-resident-for-ms must be greater than zero"
                .to_owned(),
        );
    }
    Ok(Some(requested.min(MAX_RESIDENT_READ_FOR_MS)))
}

/// Render one Raw Input transition without changing or synthesizing it.
pub fn format_resident_transition(transition: ButtonTransition) -> String {
    let edge = if transition.is_pressed() {
        "press"
    } else {
        "release"
    };
    format!(
        "resident_transition={edge} usage=0x{:02X}",
        transition.usage()
    )
}

/// Result of one finite Raw Input monitor window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidentMonitorResult {
    Timeout {
        transitions: Vec<ButtonTransition>,
    },
    Disconnect {
        transitions: Vec<ButtonTransition>,
        error: String,
    },
}

/// Keep a bounded monitor alive across idle read timeouts.
///
/// `read` returns `Ok(None)` when no Raw Input arrived during one wait.  That
/// is an idle interval, not the end of the requested monitoring window.  The
/// clock is injected so the timeout policy can be tested without HID I/O.
pub fn collect_resident_transitions_until<R, N>(
    duration_ms: u64,
    mut read: R,
    mut now_ms: N,
) -> ResidentMonitorResult
where
    R: FnMut(u64) -> Result<Option<Vec<ButtonTransition>>, String>,
    N: FnMut() -> u64,
{
    let started = now_ms();
    let deadline = started.saturating_add(duration_ms);
    let mut transitions = Vec::new();

    loop {
        let now = now_ms();
        if now >= deadline {
            return ResidentMonitorResult::Timeout { transitions };
        }

        match read(deadline.saturating_sub(now).min(i32::MAX as u64)) {
            Ok(Some(mut batch)) => transitions.append(&mut batch),
            Ok(None) => {}
            Err(error) => {
                return ResidentMonitorResult::Disconnect { transitions, error };
            }
        }
    }
}

/// Render the transfer-only portion of an authorized Apply result.
///
/// The expected report shape comes from the reviewed sequence token.  The
/// explicit `persistence=not-observed` marker prevents this line from being
/// mistaken for a current-binary USBPcap or device-readback claim.
pub fn format_verified_apply_outcome(outcome: &ApplyOutcome) -> String {
    let expected_16 = outcome
        .transfer
        .expected_report_lengths
        .iter()
        .filter(|length| **length == 16)
        .count();
    let expected_64 = outcome
        .transfer
        .expected_report_lengths
        .iter()
        .filter(|length| **length == 64)
        .count();
    let expected_other = outcome
        .transfer
        .expected_report_lengths
        .iter()
        .filter(|length| **length != 16 && **length != 64)
        .count();
    let completion = if outcome.transfer.is_complete() {
        "complete"
    } else {
        "incomplete"
    };
    format!(
        "verified_125hz_apply_transfer={completion} expected={} completed={} expected_16={expected_16} expected_64={expected_64} expected_other={expected_other} persistence={}",
        outcome.transfer.expected_reports,
        outcome.transfer.completed_reports,
        outcome.persistence.as_str(),
    )
}

/// Render a bounded post-Apply PollingRate observation.
///
/// A matching response is evidence that the current transport observed the
/// expected wire value.  It is deliberately not a persistence claim: the
/// caller still needs a current-binary reconnect/restart readback artifact
/// before changing `persistence=not-observed`.
pub fn format_polling_rate_readback(readback: &PollingRateReadback) -> String {
    format!(
        "verified_125hz_apply_readback=observed wire=0x{:02X} persistence=not-observed",
        readback.wire_value()
    )
}

/// Render a bounded selected-DPI observation from the official reconnect /
/// readback route.  The value is a profile `DPI` selection for the two
/// evidence-backed values; this line remains an observation, not a persistence
/// claim or a write authorization.
pub fn format_dpi_selection_readback(readback: &DpiSelectionReadback) -> String {
    format!(
        "verified_dpi_selection_readback=observed profile={} wire=0x{:02X} persistence=not-observed",
        readback.profile_value(),
        readback.wire_value()
    )
}

/// Parse the optional read-only PollingRate observation action.
///
/// The observation is only available as the second half of the authorized
/// 125 Hz Apply action.  It never creates a standalone frame and it is kept
/// behind its own confirmation so a normal live probe remains read-only.
pub fn parse_polling_rate_readback(args: &[String]) -> Result<bool, String> {
    if !has_flag(args, "--observe-polling-readback") {
        return Ok(false);
    }
    if !has_flag(args, "--confirm-live") {
        return Err(
            "refusing PollingRate readback: --observe-polling-readback requires --confirm-live"
                .to_owned(),
        );
    }
    if !has_flag(args, "--apply-125hz") {
        return Err(
            "refusing PollingRate readback: it requires the authorized --apply-125hz action"
                .to_owned(),
        );
    }
    if !has_flag(args, "--confirm-hid-apply") {
        return Err(
            "refusing PollingRate readback: pass --confirm-hid-apply for the preceding Apply"
                .to_owned(),
        );
    }
    if has_flag(args, "--read-resident")
        || has_flag(args, "--sendinput-zero-move")
        || has_flag(args, "--sendinput-one-pixel")
    {
        return Err(
            "refusing PollingRate readback: do not combine it with resident or SendInput actions"
                .to_owned(),
        );
    }
    Ok(true)
}

/// Parse the optional read-only selected-DPI observation action.
///
/// This route is intentionally standalone and cannot be combined with Apply,
/// resident input, or SendInput actions. It only performs a GET_REPORT on the
/// exact report-03 route proven by the controlled A/B capture. The device may
/// return an alternate short response outside the complete Apply/reconnect
/// context; that response is kept fail-closed by the device decoder.
pub fn parse_dpi_selection_readback(args: &[String]) -> Result<bool, String> {
    if !has_flag(args, "--observe-dpi-selection-readback") {
        return Ok(false);
    }
    if !has_flag(args, "--confirm-live") {
        return Err(
            "refusing DPI selection readback: --observe-dpi-selection-readback requires --confirm-live"
                .to_owned(),
        );
    }
    if has_flag(args, "--apply-125hz")
        || has_flag(args, "--observe-polling-readback")
        || has_flag(args, "--read-resident")
        || has_flag(args, "--read-resident-for-ms")
        || has_flag(args, "--sendinput-zero-move")
        || has_flag(args, "--sendinput-one-pixel")
    {
        return Err(
            "refusing DPI selection readback: do not combine it with Apply, polling readback, resident, or SendInput actions"
                .to_owned(),
        );
    }
    Ok(true)
}

/// Parse the bounded current-Rust selected-DPI Apply/reconnect action.
///
/// This action is deliberately separate from the standalone read-only probe:
/// it requires one of the two evidence-backed profile values, explicit live
/// and HID-Apply confirmation, and performs the complete ordered sequence
/// before pausing for a physical unplug/reinsert and a fresh read-only handle.
pub fn parse_dpi_apply_profile(args: &[String]) -> Result<Option<i32>, String> {
    let occurrences: Vec<usize> = args
        .iter()
        .enumerate()
        .filter_map(|(index, arg)| (arg == "--apply-dpi-profile").then_some(index))
        .collect();
    let Some(&index) = occurrences.first() else {
        return Ok(None);
    };
    if occurrences.len() > 1 {
        return Err("refusing DPI Apply: pass --apply-dpi-profile only once".to_owned());
    }
    if !has_flag(args, "--confirm-live") {
        return Err("refusing DPI Apply: --apply-dpi-profile requires --confirm-live".to_owned());
    }
    if !has_flag(args, "--confirm-hid-apply") {
        return Err(
            "refusing DPI Apply: pass --confirm-hid-apply for the complete Apply sequence"
                .to_owned(),
        );
    }
    if has_flag(args, "--apply-125hz")
        || has_flag(args, "--observe-dpi-selection-readback")
        || has_flag(args, "--observe-polling-readback")
        || has_flag(args, "--read-resident")
        || has_flag(args, "--read-resident-for-ms")
        || has_flag(args, "--sendinput-zero-move")
        || has_flag(args, "--sendinput-one-pixel")
    {
        return Err("refusing DPI Apply: do not combine it with another live action".to_owned());
    }

    let value = args
        .get(index + 1)
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| {
            "refusing DPI Apply: --apply-dpi-profile requires profile value 1 or 2".to_owned()
        })?;
    let profile_value = value
        .parse::<i32>()
        .map_err(|_| format!("refusing DPI Apply: invalid --apply-dpi-profile value {value:?}"))?;
    if !matches!(profile_value, 1 | 2) {
        return Err(format!(
            "refusing DPI Apply: profile value must be 1 or 2, got {profile_value}"
        ));
    }
    Ok(Some(profile_value))
}

/// Render the transfer-only portion of a selected-DPI Apply result.
pub fn format_dpi_apply_outcome(outcome: &ApplyOutcome, profile_value: i32) -> String {
    let expected_16 = outcome
        .transfer
        .expected_report_lengths
        .iter()
        .filter(|length| **length == 16)
        .count();
    let expected_64 = outcome
        .transfer
        .expected_report_lengths
        .iter()
        .filter(|length| **length == 64)
        .count();
    let expected_other = outcome
        .transfer
        .expected_report_lengths
        .iter()
        .filter(|length| **length != 16 && **length != 64)
        .count();
    let completion = if outcome.transfer.is_complete() {
        "complete"
    } else {
        "incomplete"
    };
    format!(
        "verified_dpi_apply_transfer={completion} profile={profile_value} wire=0x{profile_value:02X} expected={} completed={} expected_16={expected_16} expected_64={expected_64} expected_other={expected_other} persistence={}",
        outcome.transfer.expected_reports,
        outcome.transfer.completed_reports,
        outcome.persistence.as_str(),
    )
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if !has_flag(&args, "--confirm-live") {
        eprintln!(
            "refusing live probe: pass --confirm-live; available actions are \
             --read-resident, --sendinput-zero-move, --sendinput-one-pixel, \
             --observe-dpi-selection-readback, --apply-125hz --confirm-hid-apply, or \
             --apply-dpi-profile 1|2 --confirm-hid-apply"
        );
        std::process::exit(2);
    }

    let resident_read_for_ms = match parse_resident_read_for_ms(&args) {
        Ok(duration) => duration,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let observe_polling_readback = match parse_polling_rate_readback(&args) {
        Ok(observe) => observe,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let observe_dpi_selection_readback = match parse_dpi_selection_readback(&args) {
        Ok(observe) => observe,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let apply_dpi_profile = match parse_dpi_apply_profile(&args) {
        Ok(profile_value) => profile_value,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    #[cfg(not(windows))]
    {
        let _ = (
            args,
            resident_read_for_ms,
            observe_polling_readback,
            observe_dpi_selection_readback,
            apply_dpi_profile,
        );
        eprintln!("live probe is only supported on Windows");
        std::process::exit(2);
    }

    #[cfg(windows)]
    run_windows(
        &args,
        resident_read_for_ms,
        observe_polling_readback,
        observe_dpi_selection_readback,
        apply_dpi_profile,
    );
}

#[cfg(windows)]
fn run_windows(
    args: &[String],
    resident_read_for_ms: Option<u64>,
    observe_polling_readback: bool,
    observe_dpi_selection_readback: bool,
    apply_dpi_profile: Option<i32>,
) {
    use redsamurai_config::{device, resident_platform};
    use std::io::{self, Write};
    use std::thread;
    use std::time::{Duration, Instant};

    let process_identity = std::process::Command::new("whoami")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|identity| identity.trim().to_owned())
        .filter(|identity| !identity.is_empty())
        .unwrap_or_else(|| "<unavailable>".to_owned());
    println!("process_identity={process_identity}");

    let configuration = match device::discover_configuration_devices() {
        Ok(devices) => devices,
        Err(error) => {
            eprintln!("configuration HID discovery failed: {error}");
            std::process::exit(1);
        }
    };
    println!("configuration_candidates={}", configuration.len());
    for candidate in &configuration {
        println!(
            "configuration path={:?} vid={:04X} pid={:04X} usage_page={:04X} interface={}",
            candidate.path(),
            candidate.vendor_id(),
            candidate.product_id(),
            candidate.usage_page(),
            candidate.interface_number()
        );
    }

    let resident = match resident_platform::discover_resident_input_interfaces() {
        Ok(devices) => devices,
        Err(error) => {
            eprintln!("resident HID discovery failed: {error}");
            std::process::exit(1);
        }
    };
    println!("resident_candidates={}", resident.len());
    for candidate in &resident {
        println!(
            "resident path={:?} vid={:04X} pid={:04X} usage_page={:04X} usage={:04X} interface={} report_length={}",
            candidate.path,
            candidate.vendor_id,
            candidate.product_id,
            candidate.usage_page,
            candidate.usage,
            candidate.interface_number,
            candidate.input_report_length
        );
    }

    if let Some(profile_value) = apply_dpi_profile {
        if configuration.len() != 1 {
            eprintln!(
                "refusing DPI Apply: expected exactly one verified configuration collection, got {}",
                configuration.len()
            );
            std::process::exit(2);
        }

        // Keep the live action limited to the two evidence-backed fields.  A
        // full production PFD contains other unverified fields and must not
        // silently widen this authorization token.
        let profile = redsamurai_config::profile::Profile {
            slot: 1,
            doc: redsamurai_config::ini::IniDoc::parse(&format!(
                "[GROUP0]\r\nPollingRate=8\r\nDPI={profile_value}\r\n"
            )),
        };
        let plan = match redsamurai_config::device_apply::ApplyPlan::try_build_authorized(&profile)
        {
            Ok(plan) if plan.is_write_ready() && plan.warnings.is_empty() => plan,
            Ok(plan) => {
                eprintln!(
                    "refusing DPI Apply: authorization plan is not warning-free ({:?})",
                    plan.warnings
                );
                std::process::exit(2);
            }
            Err(error) => {
                eprintln!("refusing DPI Apply: authorization plan failed: {error}");
                std::process::exit(2);
            }
        };

        let outcome = {
            let mut transport = match device::open_configuration_device() {
                Ok(transport) => transport,
                Err(error) => {
                    eprintln!("configuration HID open failed: {error}");
                    std::process::exit(1);
                }
            };
            match plan.apply_verified_sequence(&mut transport) {
                Ok(outcome) => outcome,
                Err(error) => {
                    eprintln!("verified DPI={profile_value} Apply failed: {error}");
                    std::process::exit(1);
                }
            }
        };
        println!("verified_dpi_apply_sent={}", outcome.sent);
        println!("{}", format_dpi_apply_outcome(&outcome, profile_value));
        println!(
            "dpi_apply_context=complete_ordered_156_report_sequence profile={} wire=0x{:02X}",
            profile_value, profile_value
        );

        print!(
            "DPI Apply完了。マウスUSBを抜き、同じポートへ挿し直し、PnPがStatus=OKになったらEnter: "
        );
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("reconnect confirmation input failed");
            std::process::exit(1);
        }

        let reconnect_deadline = Instant::now() + Duration::from_secs(15);
        let configuration_after = loop {
            match device::discover_configuration_devices() {
                Ok(devices) if devices.len() == 1 => break devices,
                Ok(devices) if devices.is_empty() => {}
                Ok(devices) => {
                    eprintln!(
                        "refusing DPI readback: expected one configuration collection after reconnect, got {}",
                        devices.len()
                    );
                    std::process::exit(2);
                }
                Err(_) => {}
            }
            if Instant::now() >= reconnect_deadline {
                eprintln!("DPI reconnect readback timed out waiting for configuration collection");
                std::process::exit(1);
            }
            thread::sleep(Duration::from_millis(250));
        };
        println!(
            "configuration_candidates_after_reconnect={}",
            configuration_after.len()
        );
        let mut transport = match device::open_configuration_device() {
            Ok(transport) => transport,
            Err(error) => {
                eprintln!("configuration HID reopen failed: {error}");
                std::process::exit(1);
            }
        };
        let readback = match transport.observe_dpi_selection_readback() {
            Ok(readback) => readback,
            Err(error) => {
                eprintln!("DPI reconnect readback failed: {error}");
                std::process::exit(1);
            }
        };
        if !readback.matches_profile_value(profile_value)
            || readback.wire_value() != profile_value as u8
        {
            eprintln!(
                "DPI reconnect readback mismatch: expected profile={} wire=0x{:02X}, got profile={} wire=0x{:02X}",
                profile_value,
                profile_value,
                readback.profile_value(),
                readback.wire_value()
            );
            std::process::exit(1);
        }
        println!(
            "verified_dpi_reconnect_readback=observed profile={} wire=0x{:02X} persistence=observed",
            readback.profile_value(),
            readback.wire_value()
        );
    }

    if has_flag(args, "--apply-125hz") {
        if !has_flag(args, "--confirm-hid-apply") {
            eprintln!("refusing HID Apply: pass --confirm-hid-apply in addition to --confirm-live");
            std::process::exit(2);
        }
        if configuration.len() != 1 {
            eprintln!(
                "refusing HID Apply: expected exactly one verified configuration collection, got {}",
                configuration.len()
            );
            std::process::exit(2);
        }

        // This profile contains only the evidence-backed PollingRate field.
        // It cannot accidentally turn the default UI profile's unsupported
        // values into a write-ready plan.
        let profile = redsamurai_config::profile::Profile {
            slot: 1,
            doc: redsamurai_config::ini::IniDoc::parse("[GROUP0]\r\nPollingRate=8\r\n"),
        };
        let plan = match redsamurai_config::device_apply::ApplyPlan::try_build_authorized(&profile)
        {
            Ok(plan) if plan.is_write_ready() && plan.warnings.is_empty() => plan,
            Ok(plan) => {
                eprintln!(
                    "refusing HID Apply: authorization plan is not warning-free ({:?})",
                    plan.warnings
                );
                std::process::exit(2);
            }
            Err(error) => {
                eprintln!("refusing HID Apply: authorization plan failed: {error}");
                std::process::exit(2);
            }
        };
        let mut transport = match device::open_configuration_device() {
            Ok(transport) => transport,
            Err(error) => {
                eprintln!("configuration HID open failed: {error}");
                std::process::exit(1);
            }
        };
        let outcome = if observe_polling_readback {
            let (outcome, readback) =
                match plan.apply_verified_sequence_with_readback(&mut transport) {
                    Ok(result) => result,
                    Err(error) => {
                        eprintln!("verified 125 Hz Apply/readback failed: {error}");
                        std::process::exit(1);
                    }
                };
            println!("{}", format_polling_rate_readback(&readback));
            outcome
        } else {
            match plan.apply_verified_sequence(&mut transport) {
                Ok(outcome) => outcome,
                Err(error) => {
                    eprintln!("verified 125 Hz Apply failed: {error}");
                    std::process::exit(1);
                }
            }
        };
        println!("verified_125hz_apply_sent={}", outcome.sent);
        println!("{}", format_verified_apply_outcome(&outcome));
    }

    if observe_dpi_selection_readback {
        if configuration.len() != 1 {
            eprintln!(
                "refusing DPI selection readback: expected exactly one verified configuration collection, got {}",
                configuration.len()
            );
            std::process::exit(2);
        }
        let mut transport = match device::open_configuration_device() {
            Ok(transport) => transport,
            Err(error) => {
                eprintln!("configuration HID open failed: {error}");
                std::process::exit(1);
            }
        };
        let readback = match transport.observe_dpi_selection_readback() {
            Ok(readback) => readback,
            Err(error) => {
                eprintln!("DPI selection readback observation failed: {error}");
                std::process::exit(1);
            }
        };
        println!("{}", format_dpi_selection_readback(&readback));
    }

    if has_flag(args, "--read-resident") {
        if let Some(duration_ms) = resident_read_for_ms {
            if resident.len() != 1 {
                eprintln!(
                    "refusing resident monitor: expected exactly one exact resident collection, got {}",
                    resident.len()
                );
                std::process::exit(2);
            }
            println!(
                "resident_monitor=starting duration_ms={duration_ms} max_duration_ms={MAX_RESIDENT_READ_FOR_MS} path={:?}",
                resident[0].path
            );
        }

        let mut input = match resident_platform::open_resident_input_device() {
            Ok(device) => device,
            Err(error) => {
                eprintln!("resident HID open failed: {error}");
                std::process::exit(1);
            }
        };
        if let Some(duration_ms) = resident_read_for_ms {
            let monitor_succeeded = read_resident_for_ms(&mut input, duration_ms);
            // Let the Raw Input adapter unregister its message-only window
            // before turning a disconnect into the process status.
            drop(input);
            if !monitor_succeeded {
                std::process::exit(1);
            }
        } else {
            match input.read_transitions(RESIDENT_READ_ONCE_TIMEOUT_MS) {
                Ok(Some(transitions)) => {
                    for transition in transitions {
                        println!("{}", format_resident_transition(transition));
                    }
                }
                Ok(None) => println!("resident_report=timeout"),
                Err(error) => {
                    eprintln!("resident HID read failed: {error}");
                    std::process::exit(1);
                }
            }
        }
    }

    if has_flag(args, "--sendinput-zero-move") && has_flag(args, "--sendinput-one-pixel") {
        eprintln!("refusing SendInput probe: choose one mouse-move flag");
        std::process::exit(2);
    }

    let sendinput_move = if has_flag(args, "--sendinput-zero-move") {
        Some((0, 0, "zero_distance"))
    } else if has_flag(args, "--sendinput-one-pixel") {
        Some((1, 0, "one_pixel"))
    } else {
        None
    };
    if let Some((dx, dy, label)) = sendinput_move {
        resident_platform::send_mouse_event(resident_platform::MouseEvent::move_relative(dx, dy))
            .unwrap_or_else(|error| {
                eprintln!("SendInput {label} move failed: {error}");
                std::process::exit(1);
            });
        println!("sendinput_{label}=accepted");
    }
}

#[cfg(windows)]
fn read_resident_for_ms(
    input: &mut redsamurai_config::resident_platform::ResidentInputDevice,
    duration_ms: u64,
) -> bool {
    use std::time::Instant;

    let duration_ms = duration_ms.min(MAX_RESIDENT_READ_FOR_MS);
    let started = Instant::now();
    let result = collect_resident_transitions_until(
        duration_ms,
        |timeout_ms| match input.read_transitions(timeout_ms as i32) {
            Ok(result) => Ok(result),
            Err(error) => Err(error.to_string()),
        },
        || started.elapsed().as_millis().min(u64::MAX as u128) as u64,
    );

    match result {
        ResidentMonitorResult::Timeout { transitions } => {
            for transition in &transitions {
                println!("{}", format_resident_transition(*transition));
            }
            println!(
                "resident_monitor=timeout duration_ms={duration_ms} transitions={}",
                transitions.len()
            );
            true
        }
        ResidentMonitorResult::Disconnect { transitions, error } => {
            for transition in &transitions {
                println!("{}", format_resident_transition(*transition));
            }
            eprintln!(
                "resident_monitor=disconnect duration_ms={duration_ms} transitions={} error={error}",
                transitions.len()
            );
            false
        }
    }
}
