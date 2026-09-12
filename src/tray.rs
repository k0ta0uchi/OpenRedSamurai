//! Startup mode and Windows notification-area host.
//!
//! The editor and the resident service are separate lifecycles.  A normal
//! launch opens the editor; the `--tray` argument starts the resident worker
//! and owns a notification-area icon with explicit Open/Exit commands.  The
//! parser and non-Windows fallback stay usable in hardware-free tests.

use std::fmt;

/// Process startup mode selected from command-line arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupMode {
    Editor,
    Tray,
}

impl StartupMode {
    pub fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        args.into_iter()
            .skip(1)
            .any(|arg| {
                arg.as_ref()
                    .eq_ignore_ascii_case(crate::phase4::TRAY_ARGUMENT)
            })
            .then_some(Self::Tray)
            .unwrap_or(Self::Editor)
    }

    pub const fn is_tray(self) -> bool {
        matches!(self, Self::Tray)
    }
}

#[derive(Debug)]
pub enum TrayError {
    UnsupportedPlatform,
    Native(String),
    Resident(String),
}

impl fmt::Display for TrayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("tray mode is only supported on Windows"),
            Self::Native(message) => write!(f, "tray initialization failed: {message}"),
            Self::Resident(message) => write!(f, "resident service failed: {message}"),
        }
    }
}

impl std::error::Error for TrayError {}

const STOPPED_TOOLTIP: &str = "RED SAMURAI · 入力サービス停止";

#[cfg(windows)]
const TRAY_INSTANCE_MUTEX_NAME: &str = r"Local\RED_SAMURAI_16400DPI_Gaming_Mouse_Tray";

#[cfg(windows)]
#[derive(Debug)]
struct NamedMutexGuard {
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl Drop for NamedMutexGuard {
    fn drop(&mut self) {
        // Closing the last handle removes the named mutex object, so the
        // next tray launch can acquire the guard deterministically.
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.handle) };
    }
}

#[cfg(windows)]
#[derive(Debug)]
enum InstanceGuardDecision {
    Acquired(NamedMutexGuard),
    AlreadyRunning,
}

/// Create the named tray mutex and retain its handle for the caller's
/// lifetime.  A malformed name or any unexpected Win32 state is an error;
/// tray mode never falls back to running without a guard.
#[cfg(windows)]
fn acquire_named_instance(name: &str) -> Result<InstanceGuardDecision, TrayError> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, ERROR_SUCCESS};
    use windows::Win32::System::Threading::CreateMutexW;

    if name.is_empty() {
        return Err(TrayError::Native(
            "single-instance mutex name must not be empty".to_owned(),
        ));
    }

    let mut encoded_name: Vec<u16> = name.encode_utf16().collect();
    if encoded_name.contains(&0) {
        return Err(TrayError::Native(
            "single-instance mutex name must not contain NUL".to_owned(),
        ));
    }
    encoded_name.push(0);

    let handle =
        unsafe { CreateMutexW(None, false, PCWSTR(encoded_name.as_ptr())) }.map_err(|error| {
            TrayError::Native(format!("single-instance mutex creation failed: {error}"))
        })?;
    let guard = NamedMutexGuard { handle };

    match unsafe { GetLastError() } {
        ERROR_SUCCESS => Ok(InstanceGuardDecision::Acquired(guard)),
        ERROR_ALREADY_EXISTS => {
            drop(guard);
            Ok(InstanceGuardDecision::AlreadyRunning)
        }
        error => {
            drop(guard);
            Err(TrayError::Native(format!(
                "single-instance mutex returned unexpected Win32 error {}",
                error.0
            )))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerState {
    Running,
    Stopped,
}

fn notify_worker_completion<F, W>(status: Result<(), String>, publish_status: F, wake_event_loop: W)
where
    F: FnOnce(Result<(), String>),
    W: FnOnce(),
{
    // Publish first so the user-event handler's non-blocking receive observes
    // the completion that caused the wake-up.
    publish_status(status);
    wake_event_loop();
}

fn apply_worker_status(state: &mut WorkerState, status: Result<(), String>) -> Option<String> {
    match status {
        Ok(()) => None,
        Err(error) => {
            // An error reaches the tray only after the resident service has
            // exhausted its bounded recovery attempts (or failed closed).
            *state = WorkerState::Stopped;
            Some(error)
        }
    }
}

/// Start tray mode.  The implementation intentionally has no fallback that
/// opens a hidden editor on unsupported platforms.
#[cfg(windows)]
#[allow(deprecated)]
pub fn run_tray_mode() -> Result<(), TrayError> {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::thread;

    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use winit::event::Event;
    use winit::event_loop::{ControlFlow, EventLoop};

    let _instance_guard = match acquire_named_instance(TRAY_INSTANCE_MUTEX_NAME)? {
        InstanceGuardDecision::Acquired(guard) => guard,
        InstanceGuardDecision::AlreadyRunning => return Ok(()),
    };

    const OPEN_ID: &str = "redsamurai.open-editor";
    const EXIT_ID: &str = "redsamurai.exit";

    let event_loop = EventLoop::<TrayEvent>::with_user_event()
        .build()
        .map_err(|error| TrayError::Native(error.to_string()))?;
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(TrayEvent::Icon(event));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(TrayEvent::Menu(event));
    }));

    let open_item = MenuItem::with_id(OPEN_ID, "設定を開く", true, None);
    let exit_item = MenuItem::with_id(EXIT_ID, "終了", true, None);
    let menu = Menu::with_items(&[&open_item, &exit_item])
        .map_err(|error| TrayError::Native(error.to_string()))?;
    let icon = Icon::from_rgba(tray_icon_pixels()?, 32, 32)
        .map_err(|error| TrayError::Native(error.to_string()))?;
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("RED SAMURAI 常駐サービス")
        .with_icon(icon)
        .build()
        .map_err(|error| TrayError::Native(error.to_string()))?;

    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let (status_sender, status_receiver) = std::sync::mpsc::channel();
    let worker_proxy = event_loop.create_proxy();
    // `resident_service::run` owns its bounded reconnect loop.  Keep one
    // joinable thread for the whole tray lifetime so a failed Raw Input
    // session cannot overlap a replacement worker.
    let worker = thread::spawn(move || {
        let result = crate::resident_service::run(worker_stop);
        notify_worker_completion(
            result.map_err(|error| error.to_string()),
            move |status| {
                let _ = status_sender.send(status);
            },
            move || {
                let _ = worker_proxy.send_event(TrayEvent::WorkerFinished);
            },
        );
    });

    let open_id = open_item.id().clone();
    let exit_id = exit_item.id().clone();
    let event_stop = Arc::clone(&stop);
    let mut worker_state = WorkerState::Running;
    let result = event_loop.run(move |event, event_loop| {
        event_loop.set_control_flow(ControlFlow::Wait);
        match event {
            Event::UserEvent(TrayEvent::Menu(event)) if event.id == open_id => {
                if let Err(error) = launch_editor() {
                    eprintln!("{error}");
                }
            }
            Event::UserEvent(TrayEvent::Menu(event)) if event.id == exit_id => {
                event_stop.store(true, Ordering::Release);
                event_loop.exit();
            }
            Event::UserEvent(TrayEvent::Icon(event)) => {
                if matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                ) {
                    if let Err(error) = launch_editor() {
                        eprintln!("{error}");
                    }
                }
            }
            Event::UserEvent(TrayEvent::WorkerFinished) => {
                process_worker_status(&status_receiver, &mut worker_state, &tray_icon);
            }
            Event::AboutToWait => {
                process_worker_status(&status_receiver, &mut worker_state, &tray_icon);
            }
            _ => {}
        }
    });

    stop.store(true, Ordering::Release);
    let _ = worker.join();
    result.map_err(|error| TrayError::Native(error.to_string()))
}

#[cfg(windows)]
fn process_worker_status(
    status_receiver: &std::sync::mpsc::Receiver<Result<(), String>>,
    worker_state: &mut WorkerState,
    tray_icon: &tray_icon::TrayIcon,
) {
    if let Ok(status) = status_receiver.try_recv() {
        if let Some(error) = apply_worker_status(worker_state, status) {
            eprintln!("resident service stopped: {error}");
            let _ = tray_icon.set_tooltip(Some(STOPPED_TOOLTIP));
        }
    }
}

#[cfg(windows)]
#[derive(Debug)]
enum TrayEvent {
    Icon(tray_icon::TrayIconEvent),
    Menu(tray_icon::menu::MenuEvent),
    WorkerFinished,
}

#[cfg(windows)]
fn launch_editor() -> Result<(), TrayError> {
    let executable =
        std::env::current_exe().map_err(|error| TrayError::Native(error.to_string()))?;
    std::process::Command::new(executable)
        .arg("--editor")
        .spawn()
        .map(|_| ())
        .map_err(|error| TrayError::Native(error.to_string()))
}

#[cfg(windows)]
fn tray_icon_pixels() -> Result<Vec<u8>, TrayError> {
    let image = image::load_from_memory(include_bytes!("../assets/icons/redsamurai.png"))
        .map_err(|error| TrayError::Native(format!("embedded tray icon decode failed: {error}")))?
        .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    Ok(image.into_raw())
}

#[cfg(not(windows))]
pub fn run_tray_mode() -> Result<(), TrayError> {
    Err(TrayError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_mode_is_explicit_and_case_insensitive() {
        assert_eq!(
            StartupMode::from_args(["redsamurai-config", "--tray"]),
            StartupMode::Tray
        );
        assert_eq!(
            StartupMode::from_args(["redsamurai-config", "--TRAY"]),
            StartupMode::Tray
        );
        assert_eq!(
            StartupMode::from_args(["redsamurai-config", "--editor"]),
            StartupMode::Editor
        );
    }

    #[test]
    fn worker_completion_wakes_after_publishing_status() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let calls = Rc::new(RefCell::new(Vec::new()));
        let published_status = Rc::new(RefCell::new(None));
        let status_calls = Rc::clone(&calls);
        let wake_calls = Rc::clone(&calls);
        let status_slot = Rc::clone(&published_status);

        notify_worker_completion(
            Err("device unavailable".to_owned()),
            move |status| {
                status_calls.borrow_mut().push("status");
                *status_slot.borrow_mut() = Some(status);
            },
            move || wake_calls.borrow_mut().push("wake"),
        );

        assert_eq!(*calls.borrow(), vec!["status", "wake"]);
        assert_eq!(
            *published_status.borrow(),
            Some(Err("device unavailable".to_owned()))
        );
    }

    #[test]
    fn worker_failure_transitions_tray_state_to_stopped() {
        let mut state = WorkerState::Running;

        let error = apply_worker_status(&mut state, Err("device unavailable".to_owned()));

        assert_eq!(state, WorkerState::Stopped);
        assert_eq!(error.as_deref(), Some("device unavailable"));
    }

    #[test]
    fn explicit_stop_completion_keeps_tray_state_running() {
        let mut state = WorkerState::Running;

        let error = apply_worker_status(&mut state, Ok(()));

        assert_eq!(state, WorkerState::Running);
        assert!(error.is_none());
    }

    #[test]
    fn stopped_tray_state_is_sticky_after_worker_completion() {
        let mut state = WorkerState::Stopped;

        let error = apply_worker_status(&mut state, Ok(()));

        assert_eq!(state, WorkerState::Stopped);
        assert!(error.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn tray_icon_pixels_are_the_embedded_logo() {
        let pixels = tray_icon_pixels().expect("embedded tray icon must decode");

        assert_eq!(pixels.len(), 32 * 32 * 4);
        let unique_rgb = pixels
            .chunks_exact(4)
            .map(|pixel| (pixel[0], pixel[1], pixel[2]))
            .collect::<std::collections::HashSet<_>>();
        assert!(unique_rgb.len() > 4);
        assert!(pixels
            .chunks_exact(4)
            .any(|pixel| pixel[0] > 180 && pixel[1] < 110 && pixel[2] < 120));
    }

    #[cfg(windows)]
    #[test]
    fn named_instance_guard_rejects_second_owner_and_releases_after_drop() {
        let name = format!(r"Local\RED_SAMURAI_test_{}", std::process::id());

        let first = acquire_named_instance(&name).expect("first guard must be created");
        assert!(matches!(first, InstanceGuardDecision::Acquired(_)));

        let second = acquire_named_instance(&name).expect("second decision must be reported");
        assert!(matches!(second, InstanceGuardDecision::AlreadyRunning));

        drop(first);

        let reacquired = acquire_named_instance(&name).expect("guard must be reusable after drop");
        assert!(matches!(reacquired, InstanceGuardDecision::Acquired(_)));
    }

    #[cfg(windows)]
    #[test]
    fn named_instance_guard_fails_closed_for_an_empty_name() {
        let error = acquire_named_instance("").expect_err("unnamed mutexes must be rejected");

        assert!(matches!(error, TrayError::Native(message) if message.contains("name")));
    }
}
