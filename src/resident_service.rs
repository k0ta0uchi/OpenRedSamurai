//! Resident input service for tray mode.
//!
//! The service owns the long-lived composition of the platform-neutral
//! [`crate::resident::ResidentRuntime`] and the Windows input adapter.  It
//! opens only the exact read-only `MI_01` collection and emits software input
//! through `SendInput`.  A plain keyboard action that is already identical to
//! the physical HID usage is allowed to pass through natively, avoiding a
//! second foreground delivery; the configuration HID collection is never
//! opened from this module.

use std::fmt;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[cfg(windows)]
use crate::resident_platform::ButtonTransition;
use crate::resident_platform::{self, PlatformError};

/// The resident collection has two observed physical-button namespaces.  The
/// first is the bounded vendor namespace (`0x04..0x17`) used by the original
/// runtime contract.  The shipped mouse firmware currently emits the factory
/// side-key usages (`0x1E..0x27`, `0x2D`, and `0x33`/`0x34`) instead; those map
/// to profile buttons 7..18.  Keeping both mappings explicit prevents an
/// unrelated usage from invoking a profile button.
pub const FIRST_BUTTON_USAGE: u8 = 0x04;
pub const LAST_BUTTON_USAGE: u8 = FIRST_BUTTON_USAGE + crate::resident::BUTTON_COUNT - 1;

/// Read-only HID enumeration interval used as a fallback when Windows does
/// not deliver a usable `WM_INPUT_DEVICE_CHANGE` removal handle for the
/// keyboard-class collection.  The Raw Input notification remains the fast
/// path; this bounded poll closes the missed-notification case.
#[cfg(windows)]
const RESIDENT_PRESENCE_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Convert one known resident-button usage into the profile's one-based button
/// number.  Unknown usages are ignored by the resident loop.
pub const fn button_number_from_usage(usage: u8) -> Option<u8> {
    if usage >= FIRST_BUTTON_USAGE && usage <= LAST_BUTTON_USAGE {
        return Some(usage - FIRST_BUTTON_USAGE + 1);
    }
    match usage {
        0x1E..=0x27 => Some(7 + usage - 0x1E),
        0x2D => Some(17),
        // The profile table names the final side glyph as 0x34, while the
        // attached device reports the equivalent physical key as 0x33.
        0x33 | 0x34 => Some(18),
        _ => None,
    }
}

/// Service failures are surfaced to the tray owner rather than panicking the
/// process.  A device disconnect is therefore recoverable by restarting the
/// resident session in the same worker while the tray remains available.
#[derive(Debug)]
pub enum ResidentServiceError {
    Platform(PlatformError),
    UnsupportedPlatform,
    Sink(String),
}

impl fmt::Display for ResidentServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform(error) => error.fmt(f),
            Self::UnsupportedPlatform => {
                f.write_str("resident service is only supported on Windows")
            }
            Self::Sink(message) => write!(f, "resident software input failed: {message}"),
        }
    }
}

impl std::error::Error for ResidentServiceError {}

impl From<PlatformError> for ResidentServiceError {
    fn from(error: PlatformError) -> Self {
        Self::Platform(error)
    }
}

/// Bounded policy for reconnecting the resident input collection.
///
/// `max_retries` counts attempts after the initial worker session.  The
/// policy is deliberately finite: a device that cannot be reopened leaves
/// the tray in its stopped state instead of creating an unbounded retry loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryPolicy {
    max_retries: usize,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl RecoveryPolicy {
    /// Construct a bounded recovery policy.
    pub const fn new(max_retries: usize, initial_backoff: Duration, max_backoff: Duration) -> Self {
        Self {
            max_retries,
            initial_backoff,
            max_backoff,
        }
    }

    fn backoff_for_retry(self, retry_number: usize) -> Duration {
        let shift = retry_number.saturating_sub(1).min(127) as u32;
        let multiplier = 1u128.checked_shl(shift).unwrap_or(u128::MAX);
        let millis = self
            .initial_backoff
            .as_millis()
            .saturating_mul(multiplier)
            .min(self.max_backoff.as_millis())
            .min(u64::MAX as u128);
        Duration::from_millis(millis as u64)
    }
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        // Keep the retry window finite while allowing a human to unplug and
        // reconnect the mouse before the worker gives up.
        Self::new(30, Duration::from_millis(100), Duration::from_millis(1_000))
    }
}

#[cfg(windows)]
fn recovery_trace_path() -> Option<std::path::PathBuf> {
    std::env::var_os("REDSAMURAI_RECOVERY_LOG").map(std::path::PathBuf::from)
}

#[cfg(windows)]
fn write_recovery_trace(path: Option<&std::path::Path>, event: &str) {
    if path.is_none() {
        return;
    }
    // Keep service lifecycle events and Raw Input relay events behind the same
    // process-wide writer lock so concurrent worker threads cannot corrupt a
    // line in the recovery artifact.
    crate::resident_platform::write_relay_trace(event);
}

/// The single resident worker's sequential recovery actions.  Keeping retry
/// orchestration in one worker thread prevents a failed session from being
/// replaced by a duplicate worker while its Raw Input resources are still
/// shutting down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryStep {
    Run,
    Reset,
}

/// Execute one worker session and a finite number of reconnect attempts.
///
/// The caller supplies the session/reset operation so the state machine can
/// be exercised without opening HID or sending software input in tests.  A
/// reset is mandatory before every retry; if reset fails, recovery stops
/// immediately because held software input can no longer be proven safe.
fn run_recovery_loop<E, Step, Wait, IsRecoverable>(
    stop: &AtomicBool,
    policy: RecoveryPolicy,
    mut step: Step,
    mut wait: Wait,
    is_recoverable: IsRecoverable,
) -> Result<(), E>
where
    Step: FnMut(RecoveryStep) -> Result<(), E>,
    Wait: FnMut(Duration, &AtomicBool) -> bool,
    IsRecoverable: Fn(&E) -> bool,
{
    let mut retries = 0;
    loop {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }

        match step(RecoveryStep::Run) {
            Ok(()) => return Ok(()),
            Err(error) if !is_recoverable(&error) => return Err(error),
            Err(error) => {
                if stop.load(Ordering::Acquire) {
                    return Ok(());
                }
                if retries >= policy.max_retries {
                    return Err(error);
                }
                retries += 1;

                // Reset before waiting or opening another session.  This is
                // the fail-closed boundary for held synthetic inputs.
                step(RecoveryStep::Reset)?;
                if !wait(policy.backoff_for_retry(retries), stop) {
                    return Ok(());
                }
            }
        }
    }
}

fn is_recoverable_service_error(error: &ResidentServiceError) -> bool {
    matches!(
        error,
        ResidentServiceError::Platform(
            PlatformError::Discovery { .. }
                | PlatformError::InputUnavailable
                | PlatformError::InputOpen { .. }
                | PlatformError::InputRead { .. }
        )
    )
}

/// Wait for a retry while polling the shared stop flag.  The short polling
/// slice keeps tray Exit responsive even when the configured backoff grows.
fn wait_for_recovery(delay: Duration, stop: &AtomicBool) -> bool {
    const POLL_SLICE: Duration = Duration::from_millis(10);
    let started = Instant::now();
    loop {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        let elapsed = started.elapsed();
        if elapsed >= delay {
            return !stop.load(Ordering::Acquire);
        }
        std::thread::sleep((delay - elapsed).min(POLL_SLICE));
    }
}

fn finish_runtime<S>(
    runtime: &mut crate::resident::ResidentRuntime<'_, S>,
    loop_result: Result<(), ResidentServiceError>,
) -> Result<(), ResidentServiceError>
where
    S: crate::resident::InputSink,
    S::Error: fmt::Display,
{
    let cleanup_result = runtime
        .reset()
        .map_err(|error| ResidentServiceError::Sink(error.to_string()));
    match (loop_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(loop_error), Ok(())) => Err(loop_error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(loop_error), Err(cleanup_error)) => Err(ResidentServiceError::Sink(format!(
            "resident loop failed: {loop_error}; cleanup failed: {cleanup_error}"
        ))),
    }
}

#[cfg(windows)]
struct ResidentControl {
    active_profile: AtomicUsize,
    requested_profile: AtomicUsize,
}

/// Run the resident input loop until `stop` is set.
///
/// The non-Windows implementation is deliberately unavailable.  It does not
/// substitute another HID backend, which keeps the exact-collection safety
/// boundary intact in development and CI builds.
#[cfg(windows)]
pub fn run(stop: Arc<AtomicBool>) -> Result<(), ResidentServiceError> {
    use crate::macro_db::MacroDb;
    use crate::resident::ResidentRuntime;

    let recovery_log = recovery_trace_path();
    write_recovery_trace(recovery_log.as_deref(), "event=service_start");
    let profiles = load_profiles();
    let macros = MacroDb::load_default();
    let control = Arc::new(ResidentControl {
        active_profile: AtomicUsize::new(0),
        requested_profile: AtomicUsize::new(0),
    });
    let mut runtime = ResidentRuntime::new(
        &profiles[0],
        &macros,
        WindowsActionSink {
            control: Arc::clone(&control),
        },
    );
    // The first session is read-only with respect to the configuration
    // collection.  Later sessions are reconnect boundaries and may replay
    // only the typed Rust-owned sequence for the active profile.
    let mut reconnect_attempt = false;

    let loop_result = run_recovery_loop(
        &stop,
        RecoveryPolicy::default(),
        |step| {
            let step_name = match step {
                RecoveryStep::Run => "run",
                RecoveryStep::Reset => "reset",
            };
            write_recovery_trace(
                recovery_log.as_deref(),
                &format!("event=step_start step={step_name}"),
            );
            let result = match step {
                RecoveryStep::Run => {
                    let is_reconnect = reconnect_attempt;
                    reconnect_attempt = true;
                    run_device_session(
                        &stop,
                        &mut runtime,
                        &profiles,
                        &macros,
                        &control,
                        is_reconnect,
                        recovery_log.as_deref(),
                    )
                }
                RecoveryStep::Reset => runtime
                    .reset()
                    .map_err(|error| ResidentServiceError::Sink(error.to_string())),
            };
            match &result {
                Ok(()) => write_recovery_trace(
                    recovery_log.as_deref(),
                    &format!("event=step_ok step={step_name}"),
                ),
                Err(error) => write_recovery_trace(
                    recovery_log.as_deref(),
                    &format!("event=step_error step={step_name} error={error}"),
                ),
            }
            result
        },
        |delay, stop| {
            write_recovery_trace(
                recovery_log.as_deref(),
                &format!("event=backoff_start delay_ms={}", delay.as_millis()),
            );
            let proceed = wait_for_recovery(delay, stop);
            write_recovery_trace(
                recovery_log.as_deref(),
                &format!("event=backoff_end proceed={proceed}"),
            );
            proceed
        },
        is_recoverable_service_error,
    );

    write_recovery_trace(
        recovery_log.as_deref(),
        &format!("event=recovery_loop_end result={:?}", loop_result),
    );
    let result = finish_runtime(&mut runtime, loop_result);
    write_recovery_trace(
        recovery_log.as_deref(),
        &format!("event=service_end result={:?}", result),
    );
    result
}

#[cfg(windows)]
fn run_device_session<'a>(
    stop: &AtomicBool,
    runtime: &mut crate::resident::ResidentRuntime<'a, WindowsActionSink>,
    profiles: &'a [crate::profile::Profile],
    macros: &'a crate::macro_db::MacroDb,
    control: &Arc<ResidentControl>,
    reconnect: bool,
    recovery_log: Option<&std::path::Path>,
) -> Result<(), ResidentServiceError> {
    write_recovery_trace(recovery_log, "event=input_open_start");
    let mut device = match resident_platform::open_resident_input_device_for_service() {
        Ok(device) => {
            write_recovery_trace(recovery_log, "event=input_open_ok");
            device
        }
        Err(error) => {
            write_recovery_trace(
                recovery_log,
                &format!("event=input_open_error error={error}"),
            );
            return Err(error.into());
        }
    };
    if reconnect {
        reapply_rust_owned_configuration(profiles, control, stop, recovery_log);
    }
    let mut native_passthrough = NativeKeyboardPassthrough::default();
    let started = Instant::now();
    let mut next_presence_check = Instant::now() + RESIDENT_PRESENCE_POLL_INTERVAL;

    while !stop.load(Ordering::Acquire) {
        let now = Instant::now();
        if now >= next_presence_check {
            next_presence_check = now + RESIDENT_PRESENCE_POLL_INTERVAL;
            if !resident_platform::resident_input_available()? {
                write_recovery_trace(recovery_log, "event=input_presence_missing");
                return Err(ResidentServiceError::Platform(
                    PlatformError::InputUnavailable,
                ));
            }
        }
        if let Some(transitions) = device.read_transitions(50)? {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let now_ms = elapsed_ms(started);
            for transition in transitions {
                write_recovery_trace(
                    recovery_log,
                    &format!(
                        "event=transition usage=0x{:02X} edge={}",
                        transition.usage(),
                        if transition.is_pressed() {
                            "press"
                        } else {
                            "release"
                        }
                    ),
                );
                feed_transition(runtime, &mut native_passthrough, transition, now_ms)?;
            }
            runtime
                .tick(now_ms)
                .map_err(|error| ResidentServiceError::Sink(error.to_string()))?;
            switch_profile_if_requested(
                runtime,
                profiles,
                macros,
                control,
                &mut native_passthrough,
            )?;
        } else {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let now_ms = elapsed_ms(started);
            runtime
                .tick(now_ms)
                .map_err(|error| ResidentServiceError::Sink(error.to_string()))?;
            switch_profile_if_requested(
                runtime,
                profiles,
                macros,
                control,
                &mut native_passthrough,
            )?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn reapply_rust_owned_configuration(
    profiles: &[crate::profile::Profile],
    control: &ResidentControl,
    stop: &AtomicBool,
    recovery_log: Option<&std::path::Path>,
) {
    let active = control.active_profile.load(Ordering::Acquire);
    let Some(profile) = profiles.get(active) else {
        write_recovery_trace(
            recovery_log,
            &format!("event=config_reapply_skipped owner=rust reason=profile_index_{active}"),
        );
        return;
    };
    let owner = crate::device_runtime::RustOwnedConfig::from_profile(profile);
    if !owner.has_reconnect_sequence() {
        write_recovery_trace(
            recovery_log,
            "event=config_reapply_skipped owner=rust reason=no_authorized_sequence",
        );
        return;
    }
    if stop.load(Ordering::Acquire) {
        write_recovery_trace(
            recovery_log,
            "event=config_reapply_skipped owner=rust reason=stop_requested",
        );
        return;
    }

    write_recovery_trace(
        recovery_log,
        &format!(
            "event=config_reapply_start owner=rust profile={} dpi={:?}",
            active + 1,
            owner.profile_dpi_selection()
        ),
    );
    let mut configuration = match crate::device::open_configuration_device() {
        Ok(configuration) => configuration,
        Err(error) => {
            // Configuration replay is optional for the input worker.  A
            // missing configuration collection must not stop physical input
            // recovery, and the next reconnect can try again.
            write_recovery_trace(
                recovery_log,
                &format!("event=config_reapply_error owner=rust error={error}"),
            );
            return;
        }
    };

    match owner.reapply_after_reconnect(&mut configuration) {
        Ok(crate::device_runtime::ReconnectResult::Applied { sent }) => {
            write_recovery_trace(
                recovery_log,
                &format!("event=config_reapply_ok owner=rust reports={sent}"),
            );
        }
        Ok(crate::device_runtime::ReconnectResult::SkippedNoAuthorizedSequence) => {
            write_recovery_trace(
                recovery_log,
                "event=config_reapply_skipped owner=rust reason=no_authorized_sequence",
            );
        }
        Err(error) => {
            write_recovery_trace(
                recovery_log,
                &format!("event=config_reapply_error owner=rust error={error}"),
            );
        }
    }
}

#[cfg(windows)]
fn load_profiles() -> Vec<crate::profile::Profile> {
    (1..=5)
        .map(|slot| {
            crate::profile::Profile::profile_path(slot)
                .map(|path| crate::profile::Profile::load(&path, slot))
                .unwrap_or_else(|| crate::profile::Profile::default_profile(slot))
        })
        .collect()
}

#[cfg(not(windows))]
pub fn run(_stop: Arc<AtomicBool>) -> Result<(), ResidentServiceError> {
    Err(ResidentServiceError::UnsupportedPlatform)
}

#[cfg(windows)]
fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

#[cfg(windows)]
#[derive(Debug, Default)]
struct NativeKeyboardPassthrough {
    /// Buttons whose physical keyboard edge is already the requested output.
    ///
    /// The resident collection is exposed by Windows as a normal keyboard
    /// device.  For the factory single-key assignments the device therefore
    /// already delivers the requested key to the foreground application.  A
    /// second `SendInput` pair would reproduce the historical duplicate.  The
    /// ledger keeps the press/release decision stable if the profile changes
    /// while a button is held.
    held_buttons: std::collections::BTreeSet<u8>,
}

#[cfg(windows)]
impl NativeKeyboardPassthrough {
    #[cfg(test)]
    fn is_held(&self, button: u8) -> bool {
        self.held_buttons.contains(&button)
    }

    fn clear(&mut self) {
        self.held_buttons.clear();
    }

    /// The final factory side-key glyph is stored as HID usage `0x34` in the
    /// profile, while this device's keyboard collection reports the same
    /// physical key as `0x33`.  Keep this alias local to button 18; accepting
    /// it globally would turn an unrelated keyboard usage into a native
    /// pass-through.
    fn native_usage_matches(button: u8, transition_usage: u8, action_usage: u8) -> bool {
        action_usage == transition_usage
            || (button == 18 && transition_usage == 0x33 && action_usage == 0x34)
    }

    fn should_passthrough<S: crate::resident::InputSink>(
        &mut self,
        runtime: &crate::resident::ResidentRuntime<'_, S>,
        button: u8,
        transition: ButtonTransition,
    ) -> bool {
        if self.held_buttons.contains(&button) {
            if transition.is_released() {
                self.held_buttons.remove(&button);
            }
            return true;
        }

        if !transition.is_pressed() {
            return false;
        }

        let native_mapping = matches!(
            runtime.resolver().resolve(button),
            Some(crate::resident::Action::Keyboard {
                usage,
                modifiers,
            }) if Self::native_usage_matches(button, transition.usage(), usage)
                && modifiers == crate::resident::Modifiers::NONE
        );
        if native_mapping {
            self.held_buttons.insert(button);
        }
        native_mapping
    }
}

#[cfg(windows)]
fn feed_transition<S: crate::resident::InputSink<Error = PlatformError>>(
    runtime: &mut crate::resident::ResidentRuntime<'_, S>,
    native_passthrough: &mut NativeKeyboardPassthrough,
    transition: ButtonTransition,
    timestamp_ms: u64,
) -> Result<(), ResidentServiceError> {
    let Some(button) = button_number_from_usage(transition.usage()) else {
        return Ok(());
    };
    if native_passthrough.should_passthrough(runtime, button, transition) {
        write_recovery_trace(
            recovery_trace_path().as_deref(),
            &format!(
                "event=native_keyboard_passthrough button={} usage=0x{:02X} edge={}",
                button,
                transition.usage(),
                if transition.is_pressed() {
                    "press"
                } else {
                    "release"
                }
            ),
        );
        return Ok(());
    }
    runtime
        .handle(crate::resident::ButtonEvent::new(
            button,
            transition.is_pressed(),
            timestamp_ms,
        ))
        .map_err(|error| ResidentServiceError::Sink(error.to_string()))
}

#[cfg(windows)]
struct WindowsActionSink {
    control: Arc<ResidentControl>,
}

#[cfg(windows)]
impl crate::resident::InputSink for WindowsActionSink {
    type Error = PlatformError;

    fn emit(&mut self, event: crate::resident::ActionEvent) -> Result<(), Self::Error> {
        dispatch_action(event, &self.control)
    }
}

#[cfg(windows)]
fn dispatch_action(
    event: crate::resident::ActionEvent,
    control: &ResidentControl,
) -> Result<(), PlatformError> {
    use crate::resident::{Action, ActionPhase};

    match event.action {
        Action::Keyboard { usage, modifiers } => {
            send_keyboard_usage(usage, modifiers.bits(), event.phase)
        }
        Action::MouseButton(button) => send_mouse_button(button, event.phase),
        Action::MouseDoubleClick(button) => {
            if event.phase != ActionPhase::Trigger {
                return Ok(());
            }
            send_mouse_double_click(button)
        }
        Action::BasicShortcut(shortcut) => {
            let (usage, modifiers) = basic_shortcut(shortcut);
            send_keyboard_usage(usage, modifiers, event.phase)
        }
        Action::AdvancedShortcut(shortcut) => send_advanced_shortcut(shortcut, event.phase),
        Action::MediaShortcut(crate::resident::MediaShortcut::MicrophoneMute) => {
            if event.phase != ActionPhase::Trigger {
                return Ok(());
            }
            let result = crate::microphone::toggle_default_capture_mute().map_err(|error| {
                PlatformError::AudioControl {
                    message: error.to_string(),
                }
            })?;
            write_recovery_trace(
                recovery_trace_path().as_deref(),
                &format!(
                    "event=microphone_mute role={} muted={}",
                    result.role.label(),
                    result.muted
                ),
            );
            Ok(())
        }
        Action::MediaShortcut(shortcut) => {
            let Some(virtual_key) = media_virtual_key(shortcut) else {
                return Err(PlatformError::InvalidSoftwareEvent {
                    message: "microphone mute must use the Core Audio path",
                });
            };
            let pressed = matches!(event.phase, ActionPhase::Down | ActionPhase::Trigger);
            resident_platform::send_media_event(resident_platform::MediaEvent {
                virtual_key,
                pressed,
            })
        }
        Action::Fire {
            target,
            times,
            delay_ms,
        } => {
            if event.phase != ActionPhase::Trigger {
                return Ok(());
            }
            for index in 0..times.max(1) {
                send_fire_target(target)?;
                if index + 1 < times.max(1) && delay_ms != 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms as u64));
                }
            }
            Ok(())
        }
        // Device DPI switching remains a configuration-device write concern.
        // The resident path intentionally does not issue an unverified
        // feature report; the semantic trigger is safely ignored.
        Action::DpiSwitch(_) => Ok(()),
        Action::ProfileSwitch(intent) => {
            if event.phase != ActionPhase::Trigger {
                return Ok(());
            }
            request_profile_switch(intent, control);
            Ok(())
        }
        Action::MacroPlayback { .. } => Ok(()),
    }
}

#[cfg(windows)]
fn request_profile_switch(intent: crate::resident::ProfileIntent, control: &ResidentControl) {
    let current = control.active_profile.load(Ordering::Acquire);
    let next = match intent {
        crate::resident::ProfileIntent::Cycle | crate::resident::ProfileIntent::Next => {
            (current + 1) % 5
        }
        crate::resident::ProfileIntent::Previous => current.checked_sub(1).unwrap_or(4),
    };
    control.requested_profile.store(next, Ordering::Release);
}

#[cfg(windows)]
fn switch_profile_if_requested<'a>(
    runtime: &mut crate::resident::ResidentRuntime<'a, WindowsActionSink>,
    profiles: &'a [crate::profile::Profile],
    macros: &'a crate::macro_db::MacroDb,
    control: &Arc<ResidentControl>,
    native_passthrough: &mut NativeKeyboardPassthrough,
) -> Result<(), ResidentServiceError> {
    let requested = control.requested_profile.load(Ordering::Acquire);
    let active = control.active_profile.load(Ordering::Acquire);
    if requested == active || requested >= profiles.len() {
        return Ok(());
    }
    runtime
        .reset()
        .map_err(|error| ResidentServiceError::Sink(error.to_string()))?;
    native_passthrough.clear();
    control.active_profile.store(requested, Ordering::Release);
    let replacement = crate::resident::ResidentRuntime::new(
        &profiles[requested],
        macros,
        WindowsActionSink {
            control: Arc::clone(control),
        },
    );
    let old = std::mem::replace(runtime, replacement);
    drop(old);
    Ok(())
}

#[cfg(windows)]
fn send_keyboard_usage(
    usage: u8,
    modifier_bits: u8,
    phase: crate::resident::ActionPhase,
) -> Result<(), PlatformError> {
    let virtual_key = usage_to_virtual_key(usage).ok_or(PlatformError::InvalidSoftwareEvent {
        message: "profile keyboard usage is not mapped to a Windows virtual key",
    })?;
    let modifier_keys = modifier_virtual_keys(modifier_bits);
    send_keyboard_sequence(virtual_key, &modifier_keys, phase, |event| {
        resident_platform::send_keyboard_event(event)
    })
}

#[cfg(windows)]
fn send_keyboard_sequence<F>(
    virtual_key: u16,
    modifier_keys: &[u16],
    phase: crate::resident::ActionPhase,
    mut send: F,
) -> Result<(), PlatformError>
where
    F: FnMut(resident_platform::KeyboardEvent) -> Result<(), PlatformError>,
{
    match phase {
        crate::resident::ActionPhase::Down | crate::resident::ActionPhase::Trigger => {
            let mut sent_modifiers = Vec::with_capacity(modifier_keys.len());
            for key in modifier_keys.iter().copied() {
                if let Err(error) = send(resident_platform::KeyboardEvent::key_down(key)) {
                    // Treat an error as potentially late: the failed
                    // modifier record may already have reached Windows.
                    let _ = send(resident_platform::KeyboardEvent::key_up(key));
                    release_keyboard_modifiers(&sent_modifiers, &mut send);
                    return Err(error);
                }
                sent_modifiers.push(key);
            }
            if let Err(error) = send(resident_platform::KeyboardEvent::key_down(virtual_key)) {
                // SendInput can report a failed primary record after the
                // target callback has accepted it.  A best-effort key-up is
                // safe when no key-down was accepted and prevents a stuck
                // key when the failure was reported late.
                let _ = send(resident_platform::KeyboardEvent::key_up(virtual_key));
                release_keyboard_modifiers(&sent_modifiers, &mut send);
                return Err(error);
            }
            Ok(())
        }
        crate::resident::ActionPhase::Up => {
            let mut first_error = send(resident_platform::KeyboardEvent::key_up(virtual_key)).err();
            for key in modifier_keys.iter().rev().copied() {
                if let Err(error) = send(resident_platform::KeyboardEvent::key_up(key)) {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
            first_error.map_or(Ok(()), Err)
        }
    }
}

#[cfg(windows)]
fn release_keyboard_modifiers<F>(keys: &[u16], send: &mut F)
where
    F: FnMut(resident_platform::KeyboardEvent) -> Result<(), PlatformError>,
{
    for key in keys.iter().rev().copied() {
        let _ = send(resident_platform::KeyboardEvent::key_up(key));
    }
}

#[cfg(windows)]
fn modifier_virtual_keys(bits: u8) -> Vec<u16> {
    let mut keys = Vec::with_capacity(4);
    if bits & crate::resident::Modifiers::CTRL.bits() != 0 {
        keys.push(0x11);
    }
    if bits & crate::resident::Modifiers::ALT.bits() != 0 {
        keys.push(0x12);
    }
    if bits & crate::resident::Modifiers::SHIFT.bits() != 0 {
        keys.push(0x10);
    }
    if bits & crate::resident::Modifiers::WIN.bits() != 0 {
        keys.push(0x5B);
    }
    keys
}

#[cfg(windows)]
fn send_mouse_button(
    button: crate::resident::MouseButton,
    phase: crate::resident::ActionPhase,
) -> Result<(), PlatformError> {
    let button = platform_mouse_button(button);
    resident_platform::send_mouse_event(resident_platform::MouseEvent::button(
        button,
        matches!(
            phase,
            crate::resident::ActionPhase::Down | crate::resident::ActionPhase::Trigger
        ),
    ))
}

#[cfg(windows)]
fn platform_mouse_button(button: crate::resident::MouseButton) -> resident_platform::MouseButton {
    match button {
        crate::resident::MouseButton::Left => resident_platform::MouseButton::Left,
        crate::resident::MouseButton::Right => resident_platform::MouseButton::Right,
        crate::resident::MouseButton::Middle => resident_platform::MouseButton::Middle,
        crate::resident::MouseButton::Back => resident_platform::MouseButton::X1,
        crate::resident::MouseButton::Forward => resident_platform::MouseButton::X2,
        crate::resident::MouseButton::X1 => resident_platform::MouseButton::X1,
        crate::resident::MouseButton::X2 => resident_platform::MouseButton::X2,
    }
}

#[cfg(windows)]
fn send_mouse_sequence<F>(
    button: crate::resident::MouseButton,
    pressed_edges: &[bool],
    mut send: F,
) -> Result<(), PlatformError>
where
    F: FnMut(resident_platform::MouseEvent) -> Result<(), PlatformError>,
{
    let button = platform_mouse_button(button);
    let mut held = false;
    for pressed in pressed_edges.iter().copied() {
        let event = resident_platform::MouseEvent::button(button, pressed);
        if let Err(error) = send(event) {
            // A composite action has no runtime-held entry to be retried by
            // ResidentRuntime::reset.  Release a successfully sent press
            // before returning the original SendInput error instead.
            if held || pressed {
                let _ = send(resident_platform::MouseEvent::button(button, false));
            }
            return Err(error);
        }
        held = pressed;
    }
    Ok(())
}

#[cfg(windows)]
fn send_mouse_double_click(button: crate::resident::MouseButton) -> Result<(), PlatformError> {
    send_mouse_double_click_with(button, resident_platform::send_mouse_event)
}

#[cfg(windows)]
fn send_mouse_double_click_with<F>(
    button: crate::resident::MouseButton,
    send: F,
) -> Result<(), PlatformError>
where
    F: FnMut(resident_platform::MouseEvent) -> Result<(), PlatformError>,
{
    send_mouse_sequence(button, &[true, false, true, false], send)
}

#[cfg(windows)]
fn send_fire_target(target: crate::profile::FireTarget) -> Result<(), PlatformError> {
    send_fire_target_with(target, resident_platform::send_software_event)
}

#[cfg(windows)]
fn send_fire_target_with<F>(
    target: crate::profile::FireTarget,
    mut send: F,
) -> Result<(), PlatformError>
where
    F: FnMut(resident_platform::SoftwareInputEvent) -> Result<(), PlatformError>,
{
    use crate::profile::FireTarget;
    match target {
        FireTarget::MouseLeft => {
            send_mouse_click_with(crate::resident::MouseButton::Left, |event| {
                send(resident_platform::SoftwareInputEvent::Mouse(event))
            })
        }
        FireTarget::MouseRight => {
            send_mouse_click_with(crate::resident::MouseButton::Right, |event| {
                send(resident_platform::SoftwareInputEvent::Mouse(event))
            })
        }
        FireTarget::MouseMiddle => {
            send_mouse_click_with(crate::resident::MouseButton::Middle, |event| {
                send(resident_platform::SoftwareInputEvent::Mouse(event))
            })
        }
        FireTarget::Keyboard(usage) => {
            let virtual_key =
                usage_to_virtual_key(usage).ok_or(PlatformError::InvalidSoftwareEvent {
                    message: "profile keyboard usage is not mapped to a Windows virtual key",
                })?;
            send_keyboard_sequence(
                virtual_key,
                &[],
                crate::resident::ActionPhase::Down,
                |event| send(resident_platform::SoftwareInputEvent::Keyboard(event)),
            )?;
            match send_keyboard_sequence(
                virtual_key,
                &[],
                crate::resident::ActionPhase::Up,
                |event| send(resident_platform::SoftwareInputEvent::Keyboard(event)),
            ) {
                Ok(()) => Ok(()),
                Err(error) => {
                    // The Up sequence may have failed before the key-up
                    // record was accepted.  A best-effort retry closes the
                    // one key pressed by this fire action.
                    let _ = send(resident_platform::SoftwareInputEvent::Keyboard(
                        resident_platform::KeyboardEvent::key_up(virtual_key),
                    ));
                    Err(error)
                }
            }
        }
    }
}

#[cfg(windows)]
fn send_mouse_click_with<F>(
    button: crate::resident::MouseButton,
    send: F,
) -> Result<(), PlatformError>
where
    F: FnMut(resident_platform::MouseEvent) -> Result<(), PlatformError>,
{
    send_mouse_sequence(button, &[true, false], send)
}

#[cfg(windows)]
fn basic_shortcut(shortcut: crate::resident::BasicShortcut) -> (u8, u8) {
    use crate::resident::BasicShortcut;
    let usage = match shortcut {
        BasicShortcut::Cut => 0x1B,
        BasicShortcut::Copy => 0x06,
        BasicShortcut::Paste => 0x19,
        BasicShortcut::SelectAll => 0x04,
        BasicShortcut::Find => 0x09,
        BasicShortcut::New => 0x11,
        BasicShortcut::Print => 0x13,
        BasicShortcut::Save => 0x16,
    };
    (usage, crate::resident::Modifiers::CTRL.bits())
}

#[cfg(windows)]
fn send_advanced_shortcut(
    shortcut: crate::resident::AdvancedShortcut,
    phase: crate::resident::ActionPhase,
) -> Result<(), PlatformError> {
    use crate::resident::AdvancedShortcut;
    let keyboard = match shortcut {
        AdvancedShortcut::SwitchWindow => Some((0x2B, crate::resident::Modifiers::ALT.bits())),
        AdvancedShortcut::CloseWindow => Some((0x3D, crate::resident::Modifiers::ALT.bits())),
        AdvancedShortcut::OpenWindow => Some((0x12, crate::resident::Modifiers::CTRL.bits())),
        AdvancedShortcut::Run => Some((0x15, crate::resident::Modifiers::WIN.bits())),
        AdvancedShortcut::ShowDesktop => Some((0x07, crate::resident::Modifiers::WIN.bits())),
        AdvancedShortcut::LockPc => Some((0x0F, crate::resident::Modifiers::WIN.bits())),
        _ => None,
    };
    if let Some((usage, modifiers)) = keyboard {
        return send_keyboard_usage(usage, modifiers, phase);
    }

    let virtual_key = match shortcut {
        AdvancedShortcut::BrowserHome => 0xAC,
        AdvancedShortcut::BrowserForward => 0xA7,
        AdvancedShortcut::BrowserBack => 0xA6,
        AdvancedShortcut::BrowserStop => 0xA9,
        AdvancedShortcut::BrowserRefresh => 0xA8,
        AdvancedShortcut::BrowserSearch => 0xAA,
        AdvancedShortcut::BrowserFavorites => 0xAB,
        AdvancedShortcut::Mail => 0xB4,
        _ => return Ok(()),
    };
    resident_platform::send_media_event(resident_platform::MediaEvent {
        virtual_key,
        pressed: matches!(
            phase,
            crate::resident::ActionPhase::Down | crate::resident::ActionPhase::Trigger
        ),
    })
}

#[cfg(windows)]
fn media_virtual_key(shortcut: crate::resident::MediaShortcut) -> Option<u16> {
    use crate::resident::MediaShortcut;
    match shortcut {
        MediaShortcut::PlayPause => Some(0xB3),
        MediaShortcut::Stop => Some(0xB2),
        MediaShortcut::Previous => Some(0xB1),
        MediaShortcut::Next => Some(0xB0),
        MediaShortcut::VolumeUp => Some(0xAF),
        MediaShortcut::VolumeDown => Some(0xAE),
        MediaShortcut::Mute => Some(0xAD),
        // Microphone mute is dispatched through Core Audio and must never be
        // encoded as the unrelated VK_LAUNCH_APP1 value.
        MediaShortcut::MicrophoneMute => None,
        MediaShortcut::MediaPlayer => Some(0xB5),
    }
}

#[cfg(windows)]
fn usage_to_virtual_key(usage: u8) -> Option<u16> {
    let key = match usage {
        0x04..=0x1D => u16::from(usage) + 0x3D,
        0x1E..=0x26 => u16::from(usage) + 0x13,
        0x27 => 0x30,
        0x28 => 0x0D,
        0x29 => 0x1B,
        0x2A => 0x08,
        0x2B => 0x09,
        0x2C => 0x20,
        0x2D => 0xBD,
        0x2E => 0xBB,
        0x2F => 0xDB,
        0x30 => 0xDD,
        0x31 => 0xDC,
        0x33 => 0xBA,
        0x34 => 0xDE,
        0x35 => 0xC0,
        0x36 => 0xBC,
        0x37 => 0xBE,
        0x38 => 0xBF,
        0x39 => 0x14,
        0x3A..=0x45 => u16::from(usage) + 0x36,
        0x46 => 0x2C,
        0x47 => 0x91,
        0x48 => 0x13,
        0x49 => 0x2D,
        0x4A => 0x24,
        0x4B => 0x21,
        0x4C => 0x2E,
        0x4D => 0x23,
        0x4E => 0x22,
        0x4F => 0x27,
        0x50 => 0x25,
        0x51 => 0x28,
        0x52 => 0x26,
        0x53 => 0x90,
        0x89 => 0xE2,
        0xE0 => 0xA2,
        0xE1 => 0xA0,
        0xE2 => 0xA4,
        0xE3 => 0x5B,
        0xE4 => 0xA3,
        0xE5 => 0xA1,
        0xE6 => 0xA5,
        0xE7 => 0x5C,
        _ => return None,
    };
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resident::{ActionEvent, ActionPhase, InputSink, ResidentRuntime};

    #[derive(Debug, Default)]
    struct RecordingSink {
        events: Vec<ActionEvent>,
    }

    impl InputSink for RecordingSink {
        type Error = &'static str;

        fn emit(&mut self, event: ActionEvent) -> Result<(), Self::Error> {
            self.events.push(event);
            Ok(())
        }
    }

    #[cfg(windows)]
    #[derive(Debug, Default)]
    struct PlatformRecordingSink {
        events: Vec<ActionEvent>,
    }

    #[cfg(windows)]
    impl InputSink for PlatformRecordingSink {
        type Error = PlatformError;

        fn emit(&mut self, event: ActionEvent) -> Result<(), Self::Error> {
            self.events.push(event);
            Ok(())
        }
    }

    #[test]
    fn vendor_button_usage_mapping_is_bounded_and_one_based() {
        assert_eq!(button_number_from_usage(FIRST_BUTTON_USAGE), Some(1));
        assert_eq!(
            button_number_from_usage(LAST_BUTTON_USAGE),
            Some(crate::resident::BUTTON_COUNT)
        );
        assert_eq!(button_number_from_usage(FIRST_BUTTON_USAGE - 1), None);
        assert_eq!(button_number_from_usage(LAST_BUTTON_USAGE + 1), None);
    }

    #[test]
    fn observed_factory_side_key_usages_map_to_side_button_slots() {
        assert_eq!(button_number_from_usage(0x1E), Some(7));
        assert_eq!(button_number_from_usage(0x27), Some(16));
        assert_eq!(button_number_from_usage(0x2D), Some(17));
        assert_eq!(button_number_from_usage(0x33), Some(18));
        assert_eq!(button_number_from_usage(0x34), Some(18));
    }

    #[cfg(windows)]
    #[test]
    fn observed_factory_side_key_uses_native_hid_delivery_without_reinjection() {
        let profile = crate::profile::Profile::default_profile(1);
        let macros = crate::macro_db::MacroDb::default();
        let mut runtime =
            ResidentRuntime::with_debounce(&profile, &macros, PlatformRecordingSink::default(), 0);
        let mut passthrough = NativeKeyboardPassthrough::default();

        feed_transition(
            &mut runtime,
            &mut passthrough,
            ButtonTransition::pressed(0x1E),
            0,
        )
        .expect("factory side key should dispatch");
        feed_transition(
            &mut runtime,
            &mut passthrough,
            ButtonTransition::released(0x1E),
            10,
        )
        .expect("factory side key release should dispatch");

        assert!(runtime.sink().events.is_empty());
        assert!(!passthrough.is_held(7));
    }

    #[cfg(windows)]
    #[test]
    fn final_factory_side_key_alias_uses_native_hid_delivery_without_reinjection() {
        let profile = crate::profile::Profile::default_profile(1);
        let macros = crate::macro_db::MacroDb::default();
        let mut runtime =
            ResidentRuntime::with_debounce(&profile, &macros, PlatformRecordingSink::default(), 0);
        let mut passthrough = NativeKeyboardPassthrough::default();

        feed_transition(
            &mut runtime,
            &mut passthrough,
            ButtonTransition::pressed(0x33),
            0,
        )
        .expect("factory side key alias should dispatch");
        feed_transition(
            &mut runtime,
            &mut passthrough,
            ButtonTransition::released(0x33),
            10,
        )
        .expect("factory side key alias release should dispatch");

        assert!(runtime.sink().events.is_empty());
        assert!(!passthrough.is_held(18));
    }

    #[cfg(windows)]
    #[test]
    fn every_observed_factory_side_key_uses_native_identity_delivery() {
        // The resident MI_01 collection reports these twelve usages for the
        // factory SIDE 7..18 assignments.  SIDE 18 is the one documented
        // device/profile alias handled by NativeKeyboardPassthrough.
        let observed = [
            (7, 0x1E),
            (8, 0x1F),
            (9, 0x20),
            (10, 0x21),
            (11, 0x22),
            (12, 0x23),
            (13, 0x24),
            (14, 0x25),
            (15, 0x26),
            (16, 0x27),
            (17, 0x2D),
            (18, 0x33),
        ];

        for (button, usage) in observed {
            let profile = crate::profile::Profile::default_profile(1);
            let macros = crate::macro_db::MacroDb::default();
            let mut runtime = ResidentRuntime::with_debounce(
                &profile,
                &macros,
                PlatformRecordingSink::default(),
                0,
            );
            let mut passthrough = NativeKeyboardPassthrough::default();

            feed_transition(
                &mut runtime,
                &mut passthrough,
                ButtonTransition::pressed(usage),
                0,
            )
            .unwrap_or_else(|error| panic!("SIDE {button} press failed: {error}"));
            feed_transition(
                &mut runtime,
                &mut passthrough,
                ButtonTransition::released(usage),
                10,
            )
            .unwrap_or_else(|error| panic!("SIDE {button} release failed: {error}"));

            assert!(
                runtime.sink().events.is_empty(),
                "factory SIDE {button} must not be reinjected"
            );
            assert!(
                !passthrough.is_held(button),
                "factory SIDE {button} ledger must close on release"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_passthrough_ledger_is_cleared_when_profile_switch_replaces_runtime() {
        let profiles = [
            crate::profile::Profile::default_profile(1),
            crate::profile::Profile::default_profile(2),
        ];
        let macros = crate::macro_db::MacroDb::default();
        let control = Arc::new(ResidentControl {
            active_profile: AtomicUsize::new(0),
            requested_profile: AtomicUsize::new(0),
        });
        let mut runtime = ResidentRuntime::new(
            &profiles[0],
            &macros,
            WindowsActionSink {
                control: Arc::clone(&control),
            },
        );
        let mut passthrough = NativeKeyboardPassthrough::default();

        feed_transition(
            &mut runtime,
            &mut passthrough,
            ButtonTransition::pressed(0x1E),
            0,
        )
        .expect("native press should be accepted before profile switch");
        assert!(passthrough.is_held(7));

        control.requested_profile.store(1, Ordering::Release);
        switch_profile_if_requested(&mut runtime, &profiles, &macros, &control, &mut passthrough)
            .expect("profile replacement should complete");

        assert_eq!(control.active_profile.load(Ordering::Acquire), 1);
        assert!(
            !passthrough.is_held(7),
            "a native ledger entry must not cross a profile boundary"
        );
        feed_transition(
            &mut runtime,
            &mut passthrough,
            ButtonTransition::released(0x1E),
            10,
        )
        .expect("late release after profile switch should fail open");
    }

    #[cfg(windows)]
    #[test]
    fn native_passthrough_does_not_claim_a_combo_mapping() {
        let mut profile = crate::profile::Profile::default_profile(1);
        profile.set_button_combo(7, 0x1E, crate::resident::Modifiers::CTRL.bits());
        let macros = crate::macro_db::MacroDb::default();
        let mut runtime =
            ResidentRuntime::with_debounce(&profile, &macros, PlatformRecordingSink::default(), 0);
        let mut passthrough = NativeKeyboardPassthrough::default();

        feed_transition(
            &mut runtime,
            &mut passthrough,
            ButtonTransition::pressed(0x1E),
            0,
        )
        .expect("combo should remain a software action");

        assert_eq!(
            runtime.sink().events,
            vec![ActionEvent::new(
                crate::resident::Action::Keyboard {
                    usage: 0x1E,
                    modifiers: crate::resident::Modifiers::CTRL,
                },
                ActionPhase::Down,
            )]
        );
        assert!(!passthrough.is_held(7));
    }

    #[cfg(windows)]
    #[test]
    fn keyboard_usage_mapping_keeps_the_international_oem_key_on_its_explicit_seam() {
        assert_eq!(usage_to_virtual_key(0x89), Some(0xE2));
        assert_eq!(usage_to_virtual_key(0xFF), None);
    }

    #[test]
    fn finishing_a_failed_loop_releases_runtime_state_before_returning() {
        let mut profile = crate::profile::Profile::default_profile(1);
        profile.set_button_single_key_usage(1, 0x04);
        let macros = crate::macro_db::MacroDb::default();
        let mut runtime =
            ResidentRuntime::with_debounce(&profile, &macros, RecordingSink::default(), 0);

        runtime
            .handle(crate::resident::ButtonEvent::new(1, true, 0))
            .expect("key down");
        let result = finish_runtime(&mut runtime, Err(ResidentServiceError::UnsupportedPlatform));

        assert!(matches!(
            result,
            Err(ResidentServiceError::UnsupportedPlatform)
        ));
        assert_eq!(
            runtime.sink().events,
            vec![
                ActionEvent::new(
                    crate::resident::Action::Keyboard {
                        usage: 0x04,
                        modifiers: crate::resident::Modifiers::NONE,
                    },
                    ActionPhase::Down,
                ),
                ActionEvent::new(
                    crate::resident::Action::Keyboard {
                        usage: 0x04,
                        modifiers: crate::resident::Modifiers::NONE,
                    },
                    ActionPhase::Up,
                ),
            ]
        );
        assert!(!runtime.debouncer().is_pressed(1));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum RecoveryTestError {
        Device,
        Reset,
        Fatal,
    }

    #[test]
    fn recovery_retries_device_errors_in_one_worker_after_resetting_inputs() {
        use std::time::Duration;

        let stop = AtomicBool::new(false);
        let mut attempts = 0;
        let mut steps = Vec::new();
        let mut waits = Vec::new();
        let result = run_recovery_loop(
            &stop,
            RecoveryPolicy::new(2, Duration::from_millis(5), Duration::from_millis(7)),
            |step| {
                steps.push(step);
                match step {
                    RecoveryStep::Run => {
                        attempts += 1;
                        if attempts < 3 {
                            Err(RecoveryTestError::Device)
                        } else {
                            Ok(())
                        }
                    }
                    RecoveryStep::Reset => Ok(()),
                }
            },
            |delay, _stop| {
                waits.push(delay);
                true
            },
            |error| matches!(error, RecoveryTestError::Device),
        );

        assert!(result.is_ok());
        assert_eq!(attempts, 3);
        assert_eq!(
            steps,
            vec![
                RecoveryStep::Run,
                RecoveryStep::Reset,
                RecoveryStep::Run,
                RecoveryStep::Reset,
                RecoveryStep::Run,
            ]
        );
        assert_eq!(
            waits,
            vec![Duration::from_millis(5), Duration::from_millis(7)]
        );
    }

    #[test]
    fn default_recovery_policy_leaves_a_finite_manual_reconnect_window() {
        let policy = RecoveryPolicy::default();

        assert_eq!(policy.max_retries, 30);
        assert_eq!(policy.backoff_for_retry(1), Duration::from_millis(100));
        assert_eq!(policy.backoff_for_retry(4), Duration::from_millis(800));
        assert_eq!(policy.backoff_for_retry(5), Duration::from_millis(1_000));
        assert_eq!(policy.backoff_for_retry(30), Duration::from_millis(1_000));
    }

    #[test]
    fn recovery_backoff_stop_cancels_before_the_next_worker_attempt() {
        use std::time::Duration;

        let stop = AtomicBool::new(false);
        let mut attempts = 0;
        let mut steps = Vec::new();
        let result = run_recovery_loop(
            &stop,
            RecoveryPolicy::new(3, Duration::from_millis(1), Duration::from_millis(4)),
            |step| {
                steps.push(step);
                match step {
                    RecoveryStep::Run => {
                        attempts += 1;
                        Err(RecoveryTestError::Device)
                    }
                    RecoveryStep::Reset => Ok(()),
                }
            },
            |_delay, stop| {
                stop.store(true, Ordering::Release);
                false
            },
            |error| matches!(error, RecoveryTestError::Device),
        );

        assert_eq!(result, Ok(()));
        assert_eq!(attempts, 1);
        assert_eq!(steps, vec![RecoveryStep::Run, RecoveryStep::Reset]);
    }

    #[test]
    fn real_recovery_wait_returns_promptly_when_stop_is_already_set() {
        use std::time::{Duration, Instant};

        let stop = AtomicBool::new(true);
        let started = Instant::now();

        assert!(!wait_for_recovery(Duration::from_secs(1), &stop));
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn recovery_exhaustion_returns_the_last_device_error_without_unbounded_retries() {
        use std::time::Duration;

        let stop = AtomicBool::new(false);
        let mut attempts = 0;
        let result = run_recovery_loop(
            &stop,
            RecoveryPolicy::new(2, Duration::from_millis(1), Duration::from_millis(1)),
            |step| match step {
                RecoveryStep::Run => {
                    attempts += 1;
                    Err(RecoveryTestError::Device)
                }
                RecoveryStep::Reset => Ok(()),
            },
            |_delay, _stop| true,
            |error| matches!(error, RecoveryTestError::Device),
        );

        assert_eq!(result, Err(RecoveryTestError::Device));
        assert_eq!(attempts, 3);
    }

    #[test]
    fn recovery_fails_closed_when_reset_before_retry_fails() {
        use std::time::Duration;

        let stop = AtomicBool::new(false);
        let mut attempts = 0;
        let mut waits = 0;
        let result = run_recovery_loop(
            &stop,
            RecoveryPolicy::new(3, Duration::from_millis(1), Duration::from_millis(1)),
            |step| match step {
                RecoveryStep::Run => {
                    attempts += 1;
                    Err(RecoveryTestError::Device)
                }
                RecoveryStep::Reset => Err(RecoveryTestError::Reset),
            },
            |_delay, _stop| {
                waits += 1;
                true
            },
            |error| matches!(error, RecoveryTestError::Device),
        );

        assert_eq!(result, Err(RecoveryTestError::Reset));
        assert_eq!(attempts, 1);
        assert_eq!(waits, 0);
    }

    #[test]
    fn recovery_does_not_retry_non_device_errors() {
        use std::time::Duration;

        let stop = AtomicBool::new(false);
        let mut steps = Vec::new();
        let result = run_recovery_loop(
            &stop,
            RecoveryPolicy::new(3, Duration::from_millis(1), Duration::from_millis(1)),
            |step| {
                steps.push(step);
                Err(RecoveryTestError::Fatal)
            },
            |_delay, _stop| panic!("fatal errors must not enter backoff"),
            |error| matches!(error, RecoveryTestError::Device),
        );

        assert_eq!(result, Err(RecoveryTestError::Fatal));
        assert_eq!(steps, vec![RecoveryStep::Run]);
    }

    #[test]
    fn only_bounded_input_open_and_read_failures_are_recoverable() {
        assert!(is_recoverable_service_error(
            &ResidentServiceError::Platform(PlatformError::InputOpen {
                message: "open failed".to_owned(),
            },)
        ));
        assert!(is_recoverable_service_error(
            &ResidentServiceError::Platform(PlatformError::InputRead {
                message: "read failed".to_owned(),
            },)
        ));
        assert!(is_recoverable_service_error(
            &ResidentServiceError::Platform(PlatformError::InputUnavailable,)
        ));
        assert!(!is_recoverable_service_error(
            &ResidentServiceError::Platform(PlatformError::SendInputFailed {
                kind: "keyboard",
                code: 5,
            },)
        ));
        assert!(!is_recoverable_service_error(&ResidentServiceError::Sink(
            "sink failed".to_owned(),
        )));
    }

    #[test]
    fn recovery_resets_held_runtime_actions_before_reopening() {
        use std::time::Duration;

        let mut profile = crate::profile::Profile::default_profile(1);
        profile.set_button_single_key_usage(1, 0x04);
        let macros = crate::macro_db::MacroDb::default();
        let mut runtime =
            ResidentRuntime::with_debounce(&profile, &macros, RecordingSink::default(), 0);
        let stop = AtomicBool::new(false);
        let mut attempts = 0;

        let result = run_recovery_loop(
            &stop,
            RecoveryPolicy::new(1, Duration::ZERO, Duration::ZERO),
            |step| match step {
                RecoveryStep::Run => {
                    attempts += 1;
                    if attempts == 1 {
                        runtime
                            .handle(crate::resident::ButtonEvent::new(1, true, 0))
                            .map_err(|error| ResidentServiceError::Sink(error.to_string()))?;
                        Err(ResidentServiceError::Platform(PlatformError::InputRead {
                            message: "device disconnected".to_owned(),
                        }))
                    } else {
                        Ok(())
                    }
                }
                RecoveryStep::Reset => runtime
                    .reset()
                    .map_err(|error| ResidentServiceError::Sink(error.to_string())),
            },
            |_delay, _stop| true,
            is_recoverable_service_error,
        );

        assert!(result.is_ok());
        assert_eq!(
            runtime.sink().events,
            vec![
                ActionEvent::new(
                    crate::resident::Action::Keyboard {
                        usage: 0x04,
                        modifiers: crate::resident::Modifiers::NONE,
                    },
                    ActionPhase::Down,
                ),
                ActionEvent::new(
                    crate::resident::Action::Keyboard {
                        usage: 0x04,
                        modifiers: crate::resident::Modifiers::NONE,
                    },
                    ActionPhase::Up,
                ),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn profile_switch_dispatch_is_trigger_only() {
        let control = ResidentControl {
            active_profile: AtomicUsize::new(0),
            requested_profile: AtomicUsize::new(0),
        };

        dispatch_action(
            ActionEvent::new(
                crate::resident::Action::ProfileSwitch(crate::resident::ProfileIntent::Next),
                ActionPhase::Up,
            ),
            &control,
        )
        .expect("non-trigger profile edge is ignored");
        assert_eq!(control.requested_profile.load(Ordering::Acquire), 0);

        dispatch_action(
            ActionEvent::new(
                crate::resident::Action::ProfileSwitch(crate::resident::ProfileIntent::Next),
                ActionPhase::Trigger,
            ),
            &control,
        )
        .expect("trigger requests a profile change");
        assert_eq!(control.requested_profile.load(Ordering::Acquire), 1);
    }

    #[cfg(windows)]
    #[test]
    fn keyboard_sequence_rolls_back_modifiers_after_a_partial_send_failure() {
        use crate::resident::ActionPhase;

        let mut sent = Vec::new();
        let mut calls = 0;
        let result = send_keyboard_sequence(0x41, &[0x11, 0x12], ActionPhase::Down, |event| {
            calls += 1;
            sent.push(event);
            if calls == 3 {
                Err(PlatformError::SendInputFailed {
                    kind: "keyboard",
                    code: 5,
                })
            } else {
                Ok(())
            }
        });

        assert!(result.is_err());
        assert_eq!(
            sent,
            vec![
                resident_platform::KeyboardEvent::key_down(0x11),
                resident_platform::KeyboardEvent::key_down(0x12),
                resident_platform::KeyboardEvent::key_down(0x41),
                resident_platform::KeyboardEvent::key_up(0x41),
                resident_platform::KeyboardEvent::key_up(0x12),
                resident_platform::KeyboardEvent::key_up(0x11),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn keyboard_sequence_releases_the_primary_key_after_its_down_fails() {
        use crate::resident::ActionPhase;

        let mut sent = Vec::new();
        let mut calls = 0;
        let result = send_keyboard_sequence(0x41, &[0x11], ActionPhase::Down, |event| {
            calls += 1;
            sent.push(event);
            if calls == 2 {
                Err(PlatformError::SendInputFailed {
                    kind: "keyboard",
                    code: 5,
                })
            } else {
                Ok(())
            }
        });

        assert!(result.is_err());
        assert_eq!(
            sent,
            vec![
                resident_platform::KeyboardEvent::key_down(0x11),
                resident_platform::KeyboardEvent::key_down(0x41),
                resident_platform::KeyboardEvent::key_up(0x41),
                resident_platform::KeyboardEvent::key_up(0x11),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn keyboard_sequence_releases_a_modifier_after_its_down_fails() {
        use crate::resident::ActionPhase;

        let mut sent = Vec::new();
        let mut calls = 0;
        let result = send_keyboard_sequence(0x41, &[0x11, 0x12], ActionPhase::Down, |event| {
            calls += 1;
            sent.push(event);
            if calls == 2 {
                Err(PlatformError::SendInputFailed {
                    kind: "keyboard",
                    code: 5,
                })
            } else {
                Ok(())
            }
        });

        assert!(result.is_err());
        assert_eq!(
            sent,
            vec![
                resident_platform::KeyboardEvent::key_down(0x11),
                resident_platform::KeyboardEvent::key_down(0x12),
                resident_platform::KeyboardEvent::key_up(0x12),
                resident_platform::KeyboardEvent::key_up(0x11),
            ]
        );
    }

    #[cfg(windows)]
    #[derive(Debug)]
    struct FailingSoftwareSender {
        events: Vec<resident_platform::SoftwareInputEvent>,
        calls: usize,
        fail_on_call: usize,
    }

    #[cfg(windows)]
    impl FailingSoftwareSender {
        fn failing_on(call: usize) -> Self {
            Self {
                events: Vec::new(),
                calls: 0,
                fail_on_call: call,
            }
        }

        fn send(
            &mut self,
            event: resident_platform::SoftwareInputEvent,
        ) -> Result<(), PlatformError> {
            self.calls += 1;
            self.events.push(event);
            if self.calls == self.fail_on_call {
                Err(PlatformError::SendInputFailed {
                    kind: "test",
                    code: 5,
                })
            } else {
                Ok(())
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn mouse_double_click_releases_after_the_first_release_fails() {
        let mut sender = FailingSoftwareSender::failing_on(2);
        let result = send_mouse_double_click_with(crate::resident::MouseButton::Left, |event| {
            sender.send(resident_platform::SoftwareInputEvent::Mouse(event))
        });

        assert!(result.is_err());
        assert_eq!(
            sender.events,
            vec![
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        true,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        false,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        false,
                    )
                ),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn mouse_double_click_releases_after_the_second_release_fails() {
        let mut sender = FailingSoftwareSender::failing_on(4);
        let result = send_mouse_double_click_with(crate::resident::MouseButton::Left, |event| {
            sender.send(resident_platform::SoftwareInputEvent::Mouse(event))
        });

        assert!(result.is_err());
        assert_eq!(
            sender.events,
            vec![
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        true,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        false,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        true,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        false,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        false,
                    )
                ),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn mouse_click_releases_after_the_press_call_fails() {
        let mut sender = FailingSoftwareSender::failing_on(1);
        let result = send_mouse_click_with(crate::resident::MouseButton::Left, |event| {
            sender.send(resident_platform::SoftwareInputEvent::Mouse(event))
        });

        assert!(result.is_err());
        assert_eq!(
            sender.events,
            vec![
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        true,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        false,
                    )
                ),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn fire_mouse_click_releases_after_send_input_fails_on_release() {
        let mut sender = FailingSoftwareSender::failing_on(2);
        let result = send_fire_target_with(crate::profile::FireTarget::MouseLeft, |event| {
            sender.send(event)
        });

        assert!(result.is_err());
        assert_eq!(
            sender.events,
            vec![
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        true,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        false,
                    )
                ),
                resident_platform::SoftwareInputEvent::Mouse(
                    resident_platform::MouseEvent::button(
                        resident_platform::MouseButton::Left,
                        false,
                    )
                ),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn fire_keyboard_click_releases_after_send_input_fails_on_release() {
        let mut sender = FailingSoftwareSender::failing_on(2);
        let result = send_fire_target_with(crate::profile::FireTarget::Keyboard(0x04), |event| {
            sender.send(event)
        });

        assert!(result.is_err());
        assert_eq!(
            sender.events,
            vec![
                resident_platform::SoftwareInputEvent::Keyboard(
                    resident_platform::KeyboardEvent::key_down(0x41)
                ),
                resident_platform::SoftwareInputEvent::Keyboard(
                    resident_platform::KeyboardEvent::key_up(0x41)
                ),
                resident_platform::SoftwareInputEvent::Keyboard(
                    resident_platform::KeyboardEvent::key_up(0x41)
                ),
            ]
        );
    }
}
