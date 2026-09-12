//! Platform-neutral resident runtime for software button assignments.
//!
//! This module deliberately stops at an [`InputSink`].  It resolves the
//! existing profile model into semantic actions, filters physical button
//! edges, and schedules macro key events, but it never opens a HID device or
//! writes a device report.  A platform adapter can translate `ActionEvent`
//! values into OS input (for example, Windows `SendInput`) without becoming
//! part of the resolver or scheduler.

use super::macro_db::{MacroAction, MacroDb};
use super::profile::{ButtonAssign, FireTarget, Profile};
use std::collections::BTreeMap;
use std::fmt;
use std::ops::{BitOr, BitOrAssign};

/// The profile format has twenty physical software-assignable buttons.
pub const BUTTON_COUNT: u8 = 20;
/// Default quiet period used by [`ResidentRuntime::new`].
pub const DEFAULT_DEBOUNCE_MS: u64 = 5;

/// A timestamped raw button sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonEvent {
    /// Physical button number, one-based to match `Profile::button`.
    pub button: u8,
    /// Raw electrical state (`true` = pressed).
    pub pressed: bool,
    /// Monotonic timestamp supplied by the caller, in milliseconds.
    pub timestamp_ms: u64,
}

impl ButtonEvent {
    pub const fn new(button: u8, pressed: bool, timestamp_ms: u64) -> Self {
        Self {
            button,
            pressed,
            timestamp_ms,
        }
    }

    pub const fn is_down(self) -> bool {
        self.pressed
    }
}

/// Alias useful to callers that distinguish raw input from the resident
/// button stream by name.
pub type InputButtonEvent = ButtonEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ButtonState {
    stable_pressed: bool,
    last_edge_ms: Option<u64>,
}

/// Small deterministic edge/debounce filter.
///
/// The filter emits the first press immediately, suppresses repeated samples
/// and opposite transitions inside `debounce_ms`, and emits a later stable
/// opposite transition.  Timestamps that move backwards are ignored.  The
/// policy is intentionally event-driven so tests and resident loops can use a
/// monotonic clock without sleeping.
#[derive(Debug, Clone)]
pub struct Debouncer {
    debounce_ms: u64,
    buttons: BTreeMap<u8, ButtonState>,
}

impl Debouncer {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            debounce_ms,
            buttons: BTreeMap::new(),
        }
    }

    pub const fn debounce_ms(&self) -> u64 {
        self.debounce_ms
    }

    /// Feed one raw sample and return it only when it is a new stable edge.
    pub fn update(&mut self, event: ButtonEvent) -> Option<ButtonEvent> {
        if event.button == 0 {
            return None;
        }

        let state = self.buttons.entry(event.button).or_insert(ButtonState {
            stable_pressed: false,
            last_edge_ms: None,
        });

        if let Some(last_edge_ms) = state.last_edge_ms {
            if event.timestamp_ms < last_edge_ms {
                return None;
            }
        }
        if event.pressed == state.stable_pressed {
            return None;
        }
        if let Some(last_edge_ms) = state.last_edge_ms {
            if event.timestamp_ms.saturating_sub(last_edge_ms) < self.debounce_ms {
                return None;
            }
        }

        state.stable_pressed = event.pressed;
        state.last_edge_ms = Some(event.timestamp_ms);
        Some(event)
    }

    /// Synonym for [`Self::update`] for input-loop call sites.
    pub fn accept(&mut self, event: ButtonEvent) -> Option<ButtonEvent> {
        self.update(event)
    }

    pub fn reset_button(&mut self, button: u8) {
        self.buttons.remove(&button);
    }

    pub fn reset(&mut self) {
        self.buttons.clear();
    }

    pub fn is_pressed(&self, button: u8) -> bool {
        self.buttons
            .get(&button)
            .map(|state| state.stable_pressed)
            .unwrap_or(false)
    }
}

/// Keyboard modifier mask used by combo assignments.
///
/// The bit layout matches the Phase 1.5 profile convention: CTRL, ALT, SHIFT
/// and WIN in bits 0 through 3.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1 << 0);
    pub const CONTROL: Self = Self::CTRL;
    pub const ALT: Self = Self(1 << 1);
    pub const SHIFT: Self = Self(1 << 2);
    pub const WIN: Self = Self(1 << 3);
    pub const WINDOWS: Self = Self::WIN;

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & 0x0F)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.0 | rhs.0)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

/// Mouse buttons understood by the OS adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    X1,
    X2,
}

/// Basic shortcut functions 16..=23 from the profile menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BasicShortcut {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    New,
    Print,
    Save,
}

/// Advanced shortcut functions 24..=37 from the profile menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdvancedShortcut {
    SwitchWindow,
    CloseWindow,
    OpenWindow,
    Run,
    ShowDesktop,
    LockPc,
    BrowserHome,
    BrowserForward,
    BrowserBack,
    BrowserStop,
    BrowserRefresh,
    BrowserSearch,
    BrowserFavorites,
    Mail,
}

/// Media shortcut functions 38..=46 from the profile menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaShortcut {
    PlayPause,
    Stop,
    Previous,
    Next,
    VolumeUp,
    VolumeDown,
    Mute,
    MicrophoneMute,
    MediaPlayer,
}

/// Semantic DPI intents.  They are intentionally not device commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DpiIntent {
    Cycle,
    Increase,
    Decrease,
}

/// Semantic profile intents.  A resident owner decides how to apply them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileIntent {
    Cycle,
    Next,
    Previous,
}

/// A resolved software action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// USB HID keyboard usage, plus the profile's combo modifier mask.
    Keyboard {
        usage: u8,
        modifiers: Modifiers,
    },
    MouseButton(MouseButton),
    MouseDoubleClick(MouseButton),
    BasicShortcut(BasicShortcut),
    AdvancedShortcut(AdvancedShortcut),
    MediaShortcut(MediaShortcut),
    /// Fire-key intent; repetition remains an OS/application policy.
    Fire {
        target: FireTarget,
        times: u8,
        delay_ms: u16,
    },
    /// Macro name to be looked up by the resident runtime.
    MacroPlayback {
        name: String,
    },
    DpiSwitch(DpiIntent),
    ProfileSwitch(ProfileIntent),
}

/// Whether a sink should press, release, or trigger an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionPhase {
    Down,
    Up,
    Trigger,
}

/// One semantic event delivered to an [`InputSink`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionEvent {
    pub action: Action,
    pub phase: ActionPhase,
}

impl ActionEvent {
    pub fn new(action: Action, phase: ActionPhase) -> Self {
        Self { action, phase }
    }
}

/// Platform adapter seam.  Implementations may translate semantic actions to
/// OS input, but the core never performs HID I/O itself.
pub trait InputSink {
    type Error;

    fn emit(&mut self, event: ActionEvent) -> Result<(), Self::Error>;

    fn send(&mut self, event: ActionEvent) -> Result<(), Self::Error> {
        self.emit(event)
    }
}

impl<F, E> InputSink for F
where
    F: FnMut(ActionEvent) -> Result<(), E>,
{
    type Error = E;

    fn emit(&mut self, event: ActionEvent) -> Result<(), Self::Error> {
        self(event)
    }
}

/// Error returned by [`ResidentRuntime`].
#[derive(Debug, PartialEq, Eq)]
pub enum ResidentError<E> {
    Sink(E),
}

impl<E> From<E> for ResidentError<E> {
    fn from(error: E) -> Self {
        Self::Sink(error)
    }
}

impl<E: fmt::Display> fmt::Display for ResidentError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sink(error) => write!(f, "input sink failed: {error}"),
        }
    }
}

/// Resolves one profile button into a semantic action.
pub struct ProfileResolver<'a> {
    profile: &'a Profile,
    macros: &'a MacroDb,
}

impl<'a> ProfileResolver<'a> {
    pub fn new(profile: &'a Profile, macros: &'a MacroDb) -> Self {
        Self { profile, macros }
    }

    pub fn profile(&self) -> &'a Profile {
        self.profile
    }

    pub fn macros(&self) -> &'a MacroDb {
        self.macros
    }

    /// Resolve a one-based profile button.  Invalid or intentionally disabled
    /// assignments return `None`.
    pub fn resolve(&self, button: u8) -> Option<Action> {
        if !(1..=BUTTON_COUNT).contains(&button) {
            return None;
        }
        let assignment = self.profile.button(button as usize);
        self.resolve_assignment(button as usize, &assignment)
    }

    /// Resolve an assignment obtained from `Profile::button` when the caller
    /// already has it.  Raw single-key usage is used when no side-key glyph is
    /// recoverable from the profile blob.
    pub fn resolve_assignment(&self, button: usize, assignment: &ButtonAssign) -> Option<Action> {
        if !(1..=BUTTON_COUNT as usize).contains(&button) {
            return None;
        }
        match assignment.func {
            1 => Some(Action::MouseButton(MouseButton::Left)),
            2 => Some(Action::MouseButton(MouseButton::Right)),
            3 => Some(Action::MouseButton(MouseButton::Middle)),
            4 => Some(Action::MouseButton(MouseButton::Forward)),
            5 => Some(Action::MouseButton(MouseButton::Back)),
            6 => self
                .single_key_usage(button, assignment)
                .map(|usage| Action::Keyboard {
                    usage,
                    modifiers: Modifiers::NONE,
                }),
            7 => Some(Action::Keyboard {
                usage: assignment.key_number,
                modifiers: Modifiers::from_bits(assignment.loop_number),
            }),
            11 => Some(Action::MouseDoubleClick(MouseButton::Left)),
            12 => Some(Action::Fire {
                target: FireTarget::from_key_number(assignment.key_number),
                times: assignment.loop_number,
                delay_ms: assignment.macro_name.trim().parse().unwrap_or(0),
            }),
            13 | 47 => Some(Action::DpiSwitch(DpiIntent::Cycle)),
            14 => Some(Action::ProfileSwitch(match assignment.key_number {
                1 => ProfileIntent::Next,
                2 => ProfileIntent::Previous,
                _ => ProfileIntent::Cycle,
            })),
            // The shipped FUNCTION_STRING assigns 16 to 無効 while the
            // reverse-engineered basic submenu also starts at 16.  Treat the
            // ambiguous value as disabled so a user selecting 無効 can never
            // synthesize an unintended Ctrl+X shortcut.
            16..=23 => basic_shortcut(assignment.func).map(Action::BasicShortcut),
            24..=37 => advanced_shortcut(assignment.func).map(Action::AdvancedShortcut),
            38..=46 => media_shortcut(assignment.func).map(Action::MediaShortcut),
            // Official profiles use 48/49 for FRONT5/FRONT6; factory
            // profiles use 52/53 for the dedicated DPI entries.  Both pairs
            // were confirmed as increase/decrease by the live direction test.
            48 | 52 => Some(Action::DpiSwitch(DpiIntent::Increase)),
            49 | 53 => Some(Action::DpiSwitch(DpiIntent::Decrease)),
            100 if !assignment.macro_name.trim().is_empty()
                && self
                    .macros
                    .macro_by_name(assignment.macro_name.trim())
                    .is_some() =>
            {
                Some(Action::MacroPlayback {
                    name: assignment.macro_name.trim().to_owned(),
                })
            }
            _ => None,
        }
    }

    fn single_key_usage(&self, button: usize, assignment: &ButtonAssign) -> Option<u8> {
        if let Some(glyph) = self.profile.button_key_glyph(button) {
            return SIDE_KEY_USAGES.get(glyph).copied();
        }
        (assignment.key_number != 0).then_some(assignment.key_number)
    }
}

const SIDE_KEY_USAGES: [u8; 12] = [
    0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x2D, 0x34,
];

fn basic_shortcut(func: u16) -> Option<BasicShortcut> {
    Some(match func {
        17 => BasicShortcut::Copy,
        18 => BasicShortcut::Paste,
        19 => BasicShortcut::SelectAll,
        20 => BasicShortcut::Find,
        21 => BasicShortcut::New,
        22 => BasicShortcut::Print,
        23 => BasicShortcut::Save,
        _ => return None,
    })
}

fn advanced_shortcut(func: u16) -> Option<AdvancedShortcut> {
    Some(match func {
        24 => AdvancedShortcut::SwitchWindow,
        25 => AdvancedShortcut::CloseWindow,
        26 => AdvancedShortcut::OpenWindow,
        27 => AdvancedShortcut::Run,
        28 => AdvancedShortcut::ShowDesktop,
        29 => AdvancedShortcut::LockPc,
        30 => AdvancedShortcut::BrowserHome,
        31 => AdvancedShortcut::BrowserForward,
        32 => AdvancedShortcut::BrowserBack,
        33 => AdvancedShortcut::BrowserStop,
        34 => AdvancedShortcut::BrowserRefresh,
        35 => AdvancedShortcut::BrowserSearch,
        36 => AdvancedShortcut::BrowserFavorites,
        37 => AdvancedShortcut::Mail,
        _ => return None,
    })
}

fn media_shortcut(func: u16) -> Option<MediaShortcut> {
    Some(match func {
        38 => MediaShortcut::PlayPause,
        39 => MediaShortcut::Stop,
        40 => MediaShortcut::Previous,
        41 => MediaShortcut::Next,
        42 => MediaShortcut::VolumeUp,
        43 => MediaShortcut::VolumeDown,
        44 => MediaShortcut::Mute,
        45 => MediaShortcut::MicrophoneMute,
        46 => MediaShortcut::MediaPlayer,
        _ => return None,
    })
}

/// A scheduler output record.  `at_ms` is the logical due time, not the time
/// at which a late caller happened to call `tick`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroEvent {
    pub at_ms: u64,
    pub action: MacroAction,
}

#[derive(Debug, Clone)]
struct MacroJob {
    actions: Vec<MacroAction>,
    repeats_left: usize,
    next_index: usize,
    next_at_ms: u64,
    sequence: u64,
}

/// Deterministic macro scheduler driven entirely by caller timestamps.
#[derive(Debug, Clone, Default)]
pub struct MacroScheduler {
    jobs: Vec<MacroJob>,
    next_sequence: u64,
    last_tick_ms: u64,
}

impl MacroScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a macro.  One or more repetitions are accepted; zero is treated
    /// as one repetition so a malformed profile cannot silently discard a
    /// button press.  The first due action is returned immediately.
    pub fn start(
        &mut self,
        actions: &[MacroAction],
        repetitions: usize,
        start_ms: u64,
    ) -> Vec<MacroEvent> {
        if actions.is_empty() {
            return Vec::new();
        }
        let start_ms = start_ms.max(self.last_tick_ms);
        self.last_tick_ms = start_ms;
        self.jobs.push(MacroJob {
            actions: actions.to_vec(),
            repeats_left: repetitions.max(1),
            next_index: 0,
            next_at_ms: start_ms,
            sequence: self.next_sequence,
        });
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.drain_due(start_ms)
    }

    /// Alias that makes call sites explicit about the timestamped start.
    pub fn start_at(
        &mut self,
        actions: &[MacroAction],
        repetitions: usize,
        start_ms: u64,
    ) -> Vec<MacroEvent> {
        self.start(actions, repetitions, start_ms)
    }

    /// Emit all actions due at or before `now_ms` in stable due-time/start
    /// order.  A backwards clock sample is clamped to the last sample.
    pub fn tick(&mut self, now_ms: u64) -> Vec<MacroEvent> {
        let now_ms = now_ms.max(self.last_tick_ms);
        self.last_tick_ms = now_ms;
        self.drain_due(now_ms)
    }

    pub fn is_idle(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn clear(&mut self) {
        self.jobs.clear();
        self.last_tick_ms = 0;
    }

    fn drain_due(&mut self, now_ms: u64) -> Vec<MacroEvent> {
        let mut output = Vec::new();
        loop {
            let Some(job_index) = self
                .jobs
                .iter()
                .enumerate()
                .filter(|(_, job)| job.next_at_ms <= now_ms)
                .min_by_key(|(_, job)| (job.next_at_ms, job.sequence))
                .map(|(index, _)| index)
            else {
                break;
            };

            let job = &mut self.jobs[job_index];
            let action = job.actions[job.next_index];
            let at_ms = job.next_at_ms;
            job.next_index += 1;
            job.next_at_ms = at_ms.saturating_add(action.delay_ms as u64);

            let finished = job.next_index == job.actions.len();
            if finished {
                if job.repeats_left > 1 {
                    job.repeats_left -= 1;
                    job.next_index = 0;
                } else {
                    self.jobs.remove(job_index);
                }
            }

            // Zero-usage records are padding in real MSMACRO files.  They
            // still advance the logical delay but never become OS key input.
            if action.usage != 0 {
                output.push(MacroEvent { at_ms, action });
            }
        }
        output
    }
}

/// Resident runtime tying debounce, profile resolution, macro scheduling and
/// the platform sink together.
pub struct ResidentRuntime<'a, S> {
    resolver: ProfileResolver<'a>,
    debouncer: Debouncer,
    scheduler: MacroScheduler,
    sink: S,
    held: BTreeMap<u8, Action>,
    macro_held: BTreeMap<u8, usize>,
}

impl<'a, S: InputSink> ResidentRuntime<'a, S> {
    pub fn new(profile: &'a Profile, macros: &'a MacroDb, sink: S) -> Self {
        Self::with_debounce(profile, macros, sink, DEFAULT_DEBOUNCE_MS)
    }

    pub fn with_debounce(
        profile: &'a Profile,
        macros: &'a MacroDb,
        sink: S,
        debounce_ms: u64,
    ) -> Self {
        Self {
            resolver: ProfileResolver::new(profile, macros),
            debouncer: Debouncer::new(debounce_ms),
            scheduler: MacroScheduler::new(),
            sink,
            held: BTreeMap::new(),
            macro_held: BTreeMap::new(),
        }
    }

    /// Feed one raw button event.  Only debounced edges affect the sink.
    pub fn handle(&mut self, event: ButtonEvent) -> Result<(), ResidentError<S::Error>> {
        let Some(edge) = self.debouncer.update(event) else {
            return Ok(());
        };
        if edge.pressed {
            self.press(edge.button, edge.timestamp_ms)
        } else {
            self.release(edge.button)
        }
    }

    /// Alias for event-loop readability.
    pub fn handle_button(&mut self, event: ButtonEvent) -> Result<(), ResidentError<S::Error>> {
        self.handle(event)
    }

    /// Advance macro playback to a monotonic logical time.
    pub fn tick(&mut self, now_ms: u64) -> Result<(), ResidentError<S::Error>> {
        let due = self.scheduler.tick(now_ms);
        self.emit_macro_events(due)
    }

    /// Release held software actions and cancel pending macro work before a
    /// profile is replaced.  This prevents a profile switch or service
    /// restart from leaving a synthetic key or mouse button held down.
    pub fn reset(&mut self) -> Result<(), ResidentError<S::Error>> {
        // Cancel future macro events before attempting release I/O.  If the
        // sink fails part-way through cleanup, a caller can retry without the
        // scheduler producing new input in the meantime.
        self.scheduler.clear();

        // Remove each entry only after its release succeeds.  Keeping a
        // failed (or not-yet-attempted) entry makes reset retryable instead of
        // silently losing the state needed to release a held OS input.
        let held_buttons: Vec<u8> = self.held.keys().copied().collect();
        for button in held_buttons {
            let Some(action) = self.held.get(&button).cloned() else {
                continue;
            };
            self.sink.emit(ActionEvent::new(action, ActionPhase::Up))?;
            self.held.remove(&button);
        }

        let macro_usages: Vec<u8> = self.macro_held.keys().copied().collect();
        for usage in macro_usages {
            self.sink.emit(ActionEvent::new(
                Action::Keyboard {
                    usage,
                    modifiers: Modifiers::NONE,
                },
                ActionPhase::Up,
            ))?;
            self.macro_held.remove(&usage);
        }

        self.debouncer.reset();
        Ok(())
    }

    pub fn resolver(&self) -> &ProfileResolver<'a> {
        &self.resolver
    }

    pub fn debouncer(&self) -> &Debouncer {
        &self.debouncer
    }

    pub fn scheduler(&self) -> &MacroScheduler {
        &self.scheduler
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    pub fn into_sink(self) -> S {
        self.sink
    }

    fn press(&mut self, button: u8, timestamp_ms: u64) -> Result<(), ResidentError<S::Error>> {
        let Some(action) = self.resolver.resolve(button) else {
            return Ok(());
        };
        match action.clone() {
            Action::MediaShortcut(MediaShortcut::MicrophoneMute) => {
                self.sink
                    .emit(ActionEvent::new(action, ActionPhase::Trigger))?;
            }
            Action::Keyboard { .. }
            | Action::MouseButton(_)
            | Action::BasicShortcut(_)
            | Action::AdvancedShortcut(_)
            | Action::MediaShortcut(_) => {
                self.sink
                    .emit(ActionEvent::new(action.clone(), ActionPhase::Down))?;
                self.held.insert(button, action);
            }
            Action::MacroPlayback { name } => {
                let Some((actions, repetitions)) =
                    self.resolver.macros().macro_by_name(&name).map(|mac| {
                        let repetitions = if mac.loop_type == 0 {
                            1
                        } else {
                            mac.loop_time.max(1) as usize
                        };
                        (mac.actions.clone(), repetitions)
                    })
                else {
                    return Ok(());
                };
                let due = self.scheduler.start(&actions, repetitions, timestamp_ms);
                self.emit_macro_events(due)?;
            }
            Action::MouseDoubleClick(_)
            | Action::Fire { .. }
            | Action::DpiSwitch(_)
            | Action::ProfileSwitch(_) => {
                self.sink
                    .emit(ActionEvent::new(action, ActionPhase::Trigger))?;
            }
        }
        Ok(())
    }

    fn release(&mut self, button: u8) -> Result<(), ResidentError<S::Error>> {
        let Some(action) = self.held.get(&button).cloned() else {
            return Ok(());
        };
        self.sink.emit(ActionEvent::new(action, ActionPhase::Up))?;
        self.held.remove(&button);
        Ok(())
    }

    fn emit_macro_events(
        &mut self,
        events: Vec<MacroEvent>,
    ) -> Result<(), ResidentError<S::Error>> {
        for event in events {
            let phase = if event.action.down {
                ActionPhase::Down
            } else {
                ActionPhase::Up
            };
            self.sink.emit(ActionEvent::new(
                Action::Keyboard {
                    usage: event.action.usage,
                    modifiers: Modifiers::NONE,
                },
                phase,
            ))?;
            if event.action.down {
                *self.macro_held.entry(event.action.usage).or_default() += 1;
            } else if let Some(count) = self.macro_held.get_mut(&event.action.usage) {
                if *count <= 1 {
                    self.macro_held.remove(&event.action.usage);
                } else {
                    *count -= 1;
                }
            }
        }
        Ok(())
    }
}

/// Windows adapters can be added without changing the core or the trait.
/// `resident_service` supplies the production SendInput adapter; no HID
/// feature-report writes belong in this module.
#[cfg(windows)]
pub trait WindowsInputAdapter: InputSink {}
