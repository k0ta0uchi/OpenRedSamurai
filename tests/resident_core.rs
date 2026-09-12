//! Focused tests for the platform-neutral resident runtime core.

#[path = "../src/funcs.rs"]
mod funcs;
#[path = "../src/ini.rs"]
mod ini;
#[path = "../src/macro_db.rs"]
mod macro_db;
#[path = "../src/profile.rs"]
mod profile;
#[path = "../src/resident.rs"]
mod resident;

use macro_db::{Macro, MacroAction, MacroDb};
use profile::{ButtonAssign, FireTarget, Profile};
use resident::{
    Action, ActionEvent, ActionPhase, AdvancedShortcut, BasicShortcut, ButtonEvent, Debouncer,
    DpiIntent, InputSink, MacroScheduler, MediaShortcut, Modifiers, ProfileIntent, ProfileResolver,
    ResidentRuntime,
};

#[derive(Debug, Default)]
struct RecordingSink {
    events: Vec<ActionEvent>,
}

impl InputSink for RecordingSink {
    type Error = ();

    fn emit(&mut self, event: ActionEvent) -> Result<(), Self::Error> {
        self.events.push(event);
        Ok(())
    }
}

#[derive(Debug, Default)]
struct FailOnCallSink {
    events: Vec<ActionEvent>,
    calls: usize,
    fail_on_call: Option<usize>,
}

impl InputSink for FailOnCallSink {
    type Error = &'static str;

    fn emit(&mut self, event: ActionEvent) -> Result<(), Self::Error> {
        self.calls += 1;
        if self.fail_on_call == Some(self.calls) {
            return Err("injected sink failure");
        }
        self.events.push(event);
        Ok(())
    }
}

fn macro_db(name: &str, actions: Vec<MacroAction>) -> MacroDb {
    MacroDb {
        dir: None,
        names: vec![name.to_owned()],
        macros: vec![Macro {
            name: name.to_owned(),
            loop_time: 1,
            def_delay_ms: 10,
            loop_type: 2,
            delay_type: 2,
            actions,
            orig_hex: String::new(),
            file_path: String::new(),
        }],
    }
}

#[test]
fn debouncer_emits_only_stable_edges() {
    let mut debouncer = Debouncer::new(5);

    assert_eq!(
        debouncer.update(ButtonEvent::new(1, true, 0)),
        Some(ButtonEvent::new(1, true, 0))
    );
    assert_eq!(debouncer.update(ButtonEvent::new(1, true, 1)), None);
    assert_eq!(debouncer.update(ButtonEvent::new(1, false, 2)), None);
    assert_eq!(
        debouncer.update(ButtonEvent::new(1, false, 8)),
        Some(ButtonEvent::new(1, false, 8))
    );
}

#[test]
fn resolver_translates_profile_assignments_without_device_io() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_combo(7, 0x04, Modifiers::CTRL.bits());
    profile.set_button_func(8, 38);
    let macros = MacroDb::default();
    let resolver = ProfileResolver::new(&profile, &macros);

    assert_eq!(
        resolver.resolve(7),
        Some(Action::Keyboard {
            usage: 0x04,
            modifiers: Modifiers::CTRL,
        })
    );
    assert_eq!(
        resolver.resolve(8),
        Some(Action::MediaShortcut(resident::MediaShortcut::PlayPause))
    );
}

#[test]
fn microphone_mute_is_a_trigger_and_does_not_hold_a_media_key() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_func(7, 45);
    let macros = MacroDb::default();
    let mut runtime =
        ResidentRuntime::with_debounce(&profile, &macros, RecordingSink::default(), 0);

    runtime
        .handle(ButtonEvent::new(7, true, 0))
        .expect("microphone mute press");
    runtime
        .handle(ButtonEvent::new(7, false, 1))
        .expect("microphone mute release");

    assert_eq!(
        runtime.sink().events,
        vec![ActionEvent::new(
            Action::MediaShortcut(MediaShortcut::MicrophoneMute),
            ActionPhase::Trigger,
        )]
    );
}

#[test]
fn resolver_keeps_contract_disable_id_16_fail_closed() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_func(7, 16);
    let macros = MacroDb::default();
    let resolver = ProfileResolver::new(&profile, &macros);

    assert_eq!(resolver.resolve(7), None);
}

#[test]
fn resolver_covers_the_complete_safe_action_matrix() {
    let profile = Profile::default_profile(1);
    let macros = macro_db("matrix", Vec::new());
    let resolver = ProfileResolver::new(&profile, &macros);
    let assignment = |func, loop_number, key_number, macro_name: &str| ButtonAssign {
        button_id: 1,
        func,
        loop_number,
        key_number,
        macro_name: macro_name.to_owned(),
    };

    let mut cases = vec![
        (
            assignment(1, 0, 0, ""),
            Action::MouseButton(resident::MouseButton::Left),
        ),
        (
            assignment(2, 0, 0, ""),
            Action::MouseButton(resident::MouseButton::Right),
        ),
        (
            assignment(3, 0, 0, ""),
            Action::MouseButton(resident::MouseButton::Middle),
        ),
        (
            assignment(4, 0, 0, ""),
            Action::MouseButton(resident::MouseButton::Forward),
        ),
        (
            assignment(5, 0, 0, ""),
            Action::MouseButton(resident::MouseButton::Back),
        ),
        (
            assignment(6, 0, 0x04, ""),
            Action::Keyboard {
                usage: 0x04,
                modifiers: Modifiers::NONE,
            },
        ),
        (
            assignment(
                7,
                Modifiers::CTRL.bits() | Modifiers::SHIFT.bits(),
                0x04,
                "",
            ),
            Action::Keyboard {
                usage: 0x04,
                modifiers: Modifiers::CTRL | Modifiers::SHIFT,
            },
        ),
        (
            assignment(11, 0, 0, ""),
            Action::MouseDoubleClick(resident::MouseButton::Left),
        ),
        (
            assignment(12, 2, FireTarget::Keyboard(0x04).to_key_number(), "7"),
            Action::Fire {
                target: FireTarget::Keyboard(0x04),
                times: 2,
                delay_ms: 7,
            },
        ),
        (
            assignment(13, 0, 0, ""),
            Action::DpiSwitch(DpiIntent::Cycle),
        ),
        (
            assignment(47, 0, 0, ""),
            Action::DpiSwitch(DpiIntent::Cycle),
        ),
        (
            assignment(52, 0, 0, ""),
            Action::DpiSwitch(DpiIntent::Increase),
        ),
        (
            assignment(53, 0, 0, ""),
            Action::DpiSwitch(DpiIntent::Decrease),
        ),
        (
            assignment(48, 0, 0, ""),
            Action::DpiSwitch(DpiIntent::Increase),
        ),
        (
            assignment(49, 0, 0, ""),
            Action::DpiSwitch(DpiIntent::Decrease),
        ),
        (
            assignment(14, 0, 0, ""),
            Action::ProfileSwitch(ProfileIntent::Cycle),
        ),
        (
            assignment(14, 0, 1, ""),
            Action::ProfileSwitch(ProfileIntent::Next),
        ),
        (
            assignment(14, 0, 2, ""),
            Action::ProfileSwitch(ProfileIntent::Previous),
        ),
        (
            assignment(100, 0, 0, "matrix"),
            Action::MacroPlayback {
                name: "matrix".to_owned(),
            },
        ),
    ];

    cases.extend(
        (17..=23)
            .zip([
                BasicShortcut::Copy,
                BasicShortcut::Paste,
                BasicShortcut::SelectAll,
                BasicShortcut::Find,
                BasicShortcut::New,
                BasicShortcut::Print,
                BasicShortcut::Save,
            ])
            .map(|(func, shortcut)| (assignment(func, 0, 0, ""), Action::BasicShortcut(shortcut))),
    );
    cases.extend(
        (24..=37)
            .zip([
                AdvancedShortcut::SwitchWindow,
                AdvancedShortcut::CloseWindow,
                AdvancedShortcut::OpenWindow,
                AdvancedShortcut::Run,
                AdvancedShortcut::ShowDesktop,
                AdvancedShortcut::LockPc,
                AdvancedShortcut::BrowserHome,
                AdvancedShortcut::BrowserForward,
                AdvancedShortcut::BrowserBack,
                AdvancedShortcut::BrowserStop,
                AdvancedShortcut::BrowserRefresh,
                AdvancedShortcut::BrowserSearch,
                AdvancedShortcut::BrowserFavorites,
                AdvancedShortcut::Mail,
            ])
            .map(|(func, shortcut)| {
                (
                    assignment(func, 0, 0, ""),
                    Action::AdvancedShortcut(shortcut),
                )
            }),
    );
    cases.extend(
        (38..=46)
            .zip([
                MediaShortcut::PlayPause,
                MediaShortcut::Stop,
                MediaShortcut::Previous,
                MediaShortcut::Next,
                MediaShortcut::VolumeUp,
                MediaShortcut::VolumeDown,
                MediaShortcut::Mute,
                MediaShortcut::MicrophoneMute,
                MediaShortcut::MediaPlayer,
            ])
            .map(|(func, shortcut)| (assignment(func, 0, 0, ""), Action::MediaShortcut(shortcut))),
    );

    for (assignment, expected) in cases {
        assert_eq!(resolver.resolve_assignment(1, &assignment), Some(expected));
    }
}

#[test]
fn scheduler_is_deterministic_and_respects_macro_delays() {
    let actions = [
        MacroAction {
            down: true,
            usage: 0x04,
            delay_ms: 10,
        },
        MacroAction {
            down: false,
            usage: 0x04,
            delay_ms: 20,
        },
    ];
    let mut scheduler = MacroScheduler::new();

    assert_eq!(
        scheduler.start(&actions, 1, 100),
        vec![resident::MacroEvent {
            at_ms: 100,
            action: MacroAction {
                down: true,
                usage: 0x04,
                delay_ms: 10,
            },
        }]
    );
    assert!(scheduler.tick(109).is_empty());
    assert_eq!(
        scheduler.tick(110),
        vec![resident::MacroEvent {
            at_ms: 110,
            action: MacroAction {
                down: false,
                usage: 0x04,
                delay_ms: 20,
            },
        }]
    );
    assert!(scheduler.tick(130).is_empty());
    assert!(scheduler.is_idle());
}

#[test]
fn scheduler_clear_resets_the_time_epoch_for_reused_runtime_state() {
    let action = MacroAction {
        down: true,
        usage: 0x04,
        delay_ms: 10,
    };
    let mut scheduler = MacroScheduler::new();

    assert_eq!(
        scheduler.start(&[action], 1, 100),
        vec![resident::MacroEvent { at_ms: 100, action }]
    );
    scheduler.clear();

    assert_eq!(
        scheduler.start(&[action], 1, 0),
        vec![resident::MacroEvent { at_ms: 0, action }]
    );
}

#[test]
fn runtime_emits_held_key_edges_and_macro_playback() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_single_key_usage(7, 0x04);
    profile.set_button_macro(8, "tap");
    let macros = macro_db(
        "tap",
        vec![
            MacroAction {
                down: true,
                usage: 0x05,
                delay_ms: 3,
            },
            MacroAction {
                down: false,
                usage: 0x05,
                delay_ms: 0,
            },
        ],
    );
    let mut runtime =
        ResidentRuntime::with_debounce(&profile, &macros, RecordingSink::default(), 5);

    runtime
        .handle(ButtonEvent::new(7, true, 0))
        .expect("key down");
    runtime
        .handle(ButtonEvent::new(7, false, 10))
        .expect("key up");
    runtime
        .handle(ButtonEvent::new(8, true, 20))
        .expect("macro start");
    runtime.tick(22).expect("macro before delay");
    runtime.tick(23).expect("macro up");

    assert_eq!(
        runtime.sink().events,
        vec![
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x04,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x04,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Up,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x05,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x05,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Up,
            ),
        ]
    );
}

#[test]
fn runtime_reset_releases_every_held_family_but_not_trigger_actions() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_single_key_usage(1, 0x04);
    profile.set_button_func(2, 1);
    profile.set_button_func(3, 17);
    profile.set_button_func(4, 24);
    profile.set_button_func(5, 38);
    profile.set_button_func(6, 11);
    profile.set_button_fire(7, FireTarget::Keyboard(0x04), 1, 0);
    profile.set_button_func(8, 13);
    profile.set_button_func(9, 14);
    profile.set_button_macro(10, "hold");
    let macros = macro_db(
        "hold",
        vec![MacroAction {
            down: true,
            usage: 0x05,
            delay_ms: 100,
        }],
    );
    let mut runtime =
        ResidentRuntime::with_debounce(&profile, &macros, RecordingSink::default(), 0);

    for button in 1..=10 {
        runtime
            .handle(ButtonEvent::new(button, true, 0))
            .expect("button press");
    }

    assert_eq!(
        runtime.sink().events,
        vec![
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x04,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::MouseButton(resident::MouseButton::Left),
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::BasicShortcut(BasicShortcut::Copy),
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::AdvancedShortcut(AdvancedShortcut::SwitchWindow),
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::MediaShortcut(MediaShortcut::PlayPause),
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::MouseDoubleClick(resident::MouseButton::Left),
                ActionPhase::Trigger,
            ),
            ActionEvent::new(
                Action::Fire {
                    target: FireTarget::Keyboard(0x04),
                    times: 1,
                    delay_ms: 0,
                },
                ActionPhase::Trigger,
            ),
            ActionEvent::new(Action::DpiSwitch(DpiIntent::Cycle), ActionPhase::Trigger,),
            ActionEvent::new(
                Action::ProfileSwitch(ProfileIntent::Cycle),
                ActionPhase::Trigger,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x05,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
        ]
    );

    runtime.reset().expect("profile switch cleanup");
    assert_eq!(
        runtime
            .sink()
            .events
            .iter()
            .filter(|event| event.phase == ActionPhase::Up)
            .count(),
        6,
        "five held button actions plus one macro key must be released"
    );
    assert!(runtime.scheduler().is_idle());
    for button in 1..=10 {
        assert!(!runtime.debouncer().is_pressed(button));
    }

    // Trigger-only actions do not gain a synthetic Up event during reset.
    assert_eq!(
        runtime
            .sink()
            .events
            .iter()
            .filter(|event| event.phase == ActionPhase::Trigger)
            .count(),
        4
    );
}

#[test]
fn runtime_reset_releases_held_actions_and_cancels_debounce_state() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_single_key_usage(7, 0x04);
    let macros = MacroDb::default();
    let mut runtime = ResidentRuntime::new(&profile, &macros, RecordingSink::default());

    runtime
        .handle(ButtonEvent::new(7, true, 0))
        .expect("key down");
    runtime.reset().expect("reset releases key");
    runtime
        .handle(ButtonEvent::new(7, true, 1))
        .expect("new press after reset");

    assert_eq!(
        runtime.sink().events,
        vec![
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x04,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x04,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Up,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x04,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
        ]
    );
}

#[test]
fn profile_replacement_can_start_from_a_clean_runtime_after_reset() {
    let mut old_profile = Profile::default_profile(1);
    old_profile.set_button_single_key_usage(1, 0x04);
    let old_macros = MacroDb::default();
    let mut old_runtime =
        ResidentRuntime::with_debounce(&old_profile, &old_macros, RecordingSink::default(), 0);

    old_runtime
        .handle(ButtonEvent::new(1, true, 10))
        .expect("old profile press");
    old_runtime.reset().expect("profile switch cleanup");
    let sink = old_runtime.into_sink();

    let mut new_profile = Profile::default_profile(2);
    new_profile.set_button_single_key_usage(1, 0x05);
    let new_macros = MacroDb::default();
    let mut new_runtime = ResidentRuntime::with_debounce(&new_profile, &new_macros, sink, 0);
    new_runtime
        .handle(ButtonEvent::new(1, true, 0))
        .expect("new profile press");

    assert_eq!(
        new_runtime.sink().events,
        vec![
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x04,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x04,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Up,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x05,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
        ]
    );
}

#[test]
fn runtime_reset_releases_a_macro_key_that_has_no_scheduled_up_yet() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_macro(8, "hold");
    let macros = macro_db(
        "hold",
        vec![MacroAction {
            down: true,
            usage: 0x05,
            delay_ms: 100,
        }],
    );
    let mut runtime = ResidentRuntime::new(&profile, &macros, RecordingSink::default());

    runtime
        .handle(ButtonEvent::new(8, true, 0))
        .expect("macro start");
    runtime.reset().expect("reset releases macro key");

    assert_eq!(
        runtime.sink().events,
        vec![
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x05,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Down,
            ),
            ActionEvent::new(
                Action::Keyboard {
                    usage: 0x05,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Up,
            ),
        ]
    );
}

#[test]
fn runtime_reset_preserves_unreleased_actions_when_the_sink_fails() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_single_key_usage(1, 0x04);
    profile.set_button_single_key_usage(2, 0x05);
    let macros = MacroDb::default();
    let mut runtime = ResidentRuntime::with_debounce(
        &profile,
        &macros,
        FailOnCallSink {
            fail_on_call: Some(4),
            ..Default::default()
        },
        0,
    );

    runtime
        .handle(ButtonEvent::new(1, true, 0))
        .expect("first key down");
    runtime
        .handle(ButtonEvent::new(2, true, 1))
        .expect("second key down");

    assert!(runtime.reset().is_err());
    runtime.sink_mut().fail_on_call = None;
    runtime
        .reset()
        .expect("a later reset must release the action left pending");

    assert_eq!(
        runtime
            .sink()
            .events
            .iter()
            .filter(|event| event.phase == ActionPhase::Up)
            .count(),
        2,
        "both held keys must eventually receive one release"
    );
}

#[test]
fn runtime_release_preserves_the_action_when_the_sink_fails() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_single_key_usage(1, 0x04);
    let macros = MacroDb::default();
    let mut runtime = ResidentRuntime::with_debounce(
        &profile,
        &macros,
        FailOnCallSink {
            fail_on_call: Some(2),
            ..Default::default()
        },
        0,
    );

    runtime
        .handle(ButtonEvent::new(1, true, 0))
        .expect("key down");
    assert!(runtime.handle(ButtonEvent::new(1, false, 1)).is_err());

    runtime.sink_mut().fail_on_call = None;
    runtime
        .reset()
        .expect("reset must retry a release that previously failed");

    assert_eq!(
        runtime
            .sink()
            .events
            .iter()
            .filter(|event| event.phase == ActionPhase::Up)
            .count(),
        1,
        "a failed release must remain available for cleanup"
    );
}

#[test]
fn runtime_macro_release_preserves_key_state_when_the_sink_fails() {
    let mut profile = Profile::default_profile(1);
    profile.set_button_macro(1, "tap");
    let macros = macro_db(
        "tap",
        vec![
            MacroAction {
                down: true,
                usage: 0x04,
                delay_ms: 1,
            },
            MacroAction {
                down: false,
                usage: 0x04,
                delay_ms: 0,
            },
        ],
    );
    let mut runtime = ResidentRuntime::with_debounce(
        &profile,
        &macros,
        FailOnCallSink {
            fail_on_call: Some(2),
            ..Default::default()
        },
        0,
    );

    runtime
        .handle(ButtonEvent::new(1, true, 0))
        .expect("macro key down");
    assert!(runtime.tick(1).is_err());

    runtime.sink_mut().fail_on_call = None;
    runtime
        .reset()
        .expect("reset must retry a macro release that previously failed");

    assert_eq!(
        runtime
            .sink()
            .events
            .iter()
            .filter(|event| event.phase == ActionPhase::Up)
            .count(),
        1,
        "a failed macro release must remain available for cleanup"
    );
}
