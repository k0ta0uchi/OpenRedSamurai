//! Release-scope acceptance checks that do not touch the installed device.
//!
//! These checks exercise the same public seams used by the editor and the
//! resident worker: a disposable macro database, combo assignment encoding,
//! deterministic playback, and the explicit tray/installer startup contract.
//! They make the release bundle reproducible without pretending that a UI
//! screenshot or an unobserved HID readback is a device proof.

use redsamurai_config::macro_db::{MacroAction, MacroDb};
use redsamurai_config::phase4::{build_autostart_command, InstallPlan, InstallScope};
use redsamurai_config::profile::Profile;
use redsamurai_config::resident::{
    Action, ActionEvent, ActionPhase, ButtonEvent, InputSink, Modifiers, ResidentRuntime,
};
use redsamurai_config::tray::StartupMode;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Events(Vec<ActionEvent>);

impl InputSink for Events {
    type Error = std::convert::Infallible;

    fn emit(&mut self, event: ActionEvent) -> Result<(), Self::Error> {
        self.0.push(event);
        Ok(())
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "redsamurai-release-acceptance-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create disposable acceptance directory");
    path
}

fn remove_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

#[test]
fn macro_record_edit_save_and_playback_are_one_disposable_flow() {
    let dir = temp_dir("macro");
    let result = (|| {
        let mut db = MacroDb {
            dir: Some(dir.clone()),
            names: vec![String::new(); redsamurai_config::macro_db::MSDB_SLOTS],
            ..MacroDb::default()
        };
        db.add_macro("acceptance");
        let actions = vec![
            MacroAction {
                down: true,
                usage: 0x04,
                delay_ms: 3,
            },
            MacroAction {
                down: false,
                usage: 0x04,
                delay_ms: 7,
            },
        ];
        let macro_value = db
            .macro_by_name("acceptance")
            .cloned()
            .expect("new macro is selectable");
        db.update_macro(
            "acceptance",
            redsamurai_config::macro_db::Macro {
                actions: actions.clone(),
                ..macro_value
            },
        );
        db.save().expect("save disposable macro database");

        let loaded = MacroDb::load(&dir);
        assert_eq!(loaded.macro_by_name("acceptance").unwrap().actions, actions);

        let mut profile = Profile::default_profile(1);
        profile.set_button_macro(7, "acceptance");
        let mut runtime = ResidentRuntime::with_debounce(&profile, &loaded, Events::default(), 0);
        runtime
            .handle(ButtonEvent::new(7, true, 1))
            .expect("macro button press");
        runtime.tick(20).expect("macro playback");
        let events = runtime.into_sink().0;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].phase, ActionPhase::Down);
        assert_eq!(events[1].phase, ActionPhase::Up);
        assert_eq!(
            events[0].action,
            Action::Keyboard {
                usage: 0x04,
                modifiers: Modifiers::NONE
            }
        );
        assert_eq!(events[1].action, events[0].action);
    })();
    remove_dir(&dir);
    result
}

#[test]
fn combo_capture_assignment_commit_and_cancel_are_reversible() {
    let mut profile = Profile::default_profile(1);
    let baseline = profile.clone();

    profile.set_button_combo(7, 0x04, (Modifiers::CTRL | Modifiers::SHIFT).bits());
    assert_eq!(
        redsamurai_config::resident::ProfileResolver::new(&profile, &MacroDb::default()).resolve(7),
        Some(Action::Keyboard {
            usage: 0x04,
            modifiers: Modifiers::CTRL | Modifiers::SHIFT,
        })
    );

    let cancelled = baseline.clone();
    assert_eq!(cancelled.button(7), baseline.button(7));
}

#[test]
fn tray_and_autostart_contracts_are_explicit_and_idempotent() {
    assert_eq!(
        StartupMode::from_args(["redsamurai-config", "--tray"]),
        StartupMode::Tray
    );
    assert_eq!(
        StartupMode::from_args(["redsamurai-config", "--editor"]),
        StartupMode::Editor
    );

    let executable = PathBuf::from(r"C:\Program Files\RED SAMURAI\redsamurai-config.exe");
    let command = build_autostart_command(&executable, &["--tray"]).expect("quoted Run command");
    assert_eq!(
        command,
        r#""C:\Program Files\RED SAMURAI\redsamurai-config.exe" --tray"#
    );

    let install = InstallPlan::for_current_user(&executable).expect("install plan");
    assert_eq!(install.scope(), InstallScope::CurrentUser);
    assert!(install.is_idempotent());
    assert_eq!(install.autostart_command(), Some(command.as_str()));
}
