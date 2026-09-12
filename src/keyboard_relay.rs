//! Platform-independent state machine for an all-keyboard Raw Input relay.
//!
//! The Windows boundary may register `RIDEV_NOLEGACY` and use this module to
//! decide which Raw Input samples should be re-injected unchanged and which
//! target-device samples should become mapped output keys.  This module does
//! not register a hook, create a message loop, call `SendInput`, or otherwise
//! touch physical input.  Its output is a deterministic list of model events
//! that a platform adapter can execute.
//!
//! A mapped key-down stores a clone of the mapping in the per-source route.
//! A later key-up therefore releases the action that was actually pressed,
//! even when the profile mapping has changed in the meantime.  Output keys
//! are reference-counted across all source routes: one synthetic down is
//! emitted for the first holder and one synthetic up after the last holder is
//! released.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Raw Input's keyboard break/release flag (`RI_KEY_BREAK`).
pub const RAW_KEY_BREAK: u16 = 0x0001;
/// Raw Input's extended `E0` flag (`RI_KEY_E0`).
pub const RAW_KEY_E0: u16 = 0x0002;
/// Raw Input's extended `E1` flag (`RI_KEY_E1`).
pub const RAW_KEY_E1: u16 = 0x0004;

/// Marker written by the platform adapter to every event emitted by this
/// relay.  The marker is deliberately application-specific and is carried in
/// the model so a Raw Input/hook adapter can recognize its own reinjections.
pub const DEFAULT_INJECTION_MARKER: u64 = 0x5253_4B42_524C_4159;
/// Alias retained for adapters that call the value a self-injection marker.
pub const SELF_INJECTED_MARKER: u64 = DEFAULT_INJECTION_MARKER;

/// Opaque identity of a keyboard source.
///
/// Raw Input adapters commonly use the device path, but an adapter can use a
/// stable handle or test token by converting it to this owned identifier.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct DeviceId(String);

impl DeviceId {
    /// Construct an owned source identifier.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the identifier for logging or platform lookup.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the identifier is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<String> for DeviceId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for DeviceId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<&String> for DeviceId {
    fn from(value: &String) -> Self {
        Self::new(value.clone())
    }
}

impl From<u64> for DeviceId {
    fn from(value: u64) -> Self {
        Self::new(value.to_string())
    }
}

impl From<usize> for DeviceId {
    fn from(value: usize) -> Self {
        Self::new(value.to_string())
    }
}

/// Identity used to match a key-down with its key-up.
///
/// The break bit is intentionally excluded.  The extended bits are retained
/// because an extended and non-extended key can represent different logical
/// keys even when their virtual keys happen to match.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct InputKey {
    virtual_key: u16,
    scan_code: u16,
    extended: bool,
}

impl InputKey {
    pub const fn new(virtual_key: u16, scan_code: u16, extended: bool) -> Self {
        Self {
            virtual_key,
            scan_code,
            extended,
        }
    }

    /// Match any scan code/extended state for a virtual key.
    pub const fn virtual_key(virtual_key: u16) -> Self {
        Self::new(virtual_key, 0, false)
    }

    pub const fn from_raw(virtual_key: u16, scan_code: u16, flags: u16) -> Self {
        Self::new(
            virtual_key,
            scan_code,
            flags & (RAW_KEY_E0 | RAW_KEY_E1) != 0,
        )
    }

    pub const fn virtual_key_code(self) -> u16 {
        self.virtual_key
    }

    pub const fn scan_code(self) -> u16 {
        self.scan_code
    }

    pub const fn is_extended(self) -> bool {
        self.extended
    }
}

impl From<u16> for InputKey {
    fn from(value: u16) -> Self {
        Self::virtual_key(value)
    }
}

impl From<u8> for InputKey {
    fn from(value: u8) -> Self {
        Self::virtual_key(u16::from(value))
    }
}

/// A keyboard sample received from a Raw Input adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawKeyboardSample {
    /// Source device identity, normally the normalized Raw Input device path.
    pub device_id: DeviceId,
    /// Windows virtual key from `RAWKEYBOARD::VKey`.
    pub virtual_key: u16,
    /// Scan code from `RAWKEYBOARD::MakeCode`.
    pub scan_code: u16,
    /// Raw Input keyboard flags, including break and extended bits.
    pub flags: u16,
    /// Monotonic/event timestamp supplied by the adapter.
    pub timestamp_ms: u64,
    /// Marker carried by a synthetic sample.  Physical Raw Input samples use
    /// `None`; a platform adapter can populate this from its injection tag.
    pub injection_marker: Option<u64>,
}

impl RawKeyboardSample {
    /// Construct a sample from the fields exposed by `RAWKEYBOARD`.
    pub fn new(
        device_id: impl Into<DeviceId>,
        virtual_key: u16,
        scan_code: u16,
        flags: u16,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            virtual_key,
            scan_code,
            flags,
            timestamp_ms,
            injection_marker: None,
        }
    }

    /// Construct a physical key-down sample.
    pub fn key_down(
        device_id: impl Into<DeviceId>,
        virtual_key: u16,
        scan_code: u16,
        extended: bool,
        timestamp_ms: u64,
    ) -> Self {
        Self::new(
            device_id,
            virtual_key,
            scan_code,
            if extended { RAW_KEY_E0 } else { 0 },
            timestamp_ms,
        )
    }

    /// Construct a physical key-up sample.
    pub fn key_up(
        device_id: impl Into<DeviceId>,
        virtual_key: u16,
        scan_code: u16,
        extended: bool,
        timestamp_ms: u64,
    ) -> Self {
        Self::new(
            device_id,
            virtual_key,
            scan_code,
            RAW_KEY_BREAK | if extended { RAW_KEY_E0 } else { 0 },
            timestamp_ms,
        )
    }

    /// Attach an injection marker, returning the updated sample.
    pub fn with_injection_marker(mut self, marker: u64) -> Self {
        self.injection_marker = Some(marker);
        self
    }

    /// Alias for callers that use the Windows `dwExtraInfo` terminology.
    pub fn with_marker(self, marker: u64) -> Self {
        self.with_injection_marker(marker)
    }

    pub const fn is_down(&self) -> bool {
        self.flags & RAW_KEY_BREAK == 0
    }

    pub const fn is_up(&self) -> bool {
        !self.is_down()
    }

    pub const fn is_injected(&self) -> bool {
        self.injection_marker.is_some()
    }

    pub const fn injection_marker(&self) -> Option<u64> {
        self.injection_marker
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// Alias for source-oriented adapters.
    pub fn source(&self) -> &DeviceId {
        self.device_id()
    }

    pub const fn input_key(&self) -> InputKey {
        InputKey::from_raw(self.virtual_key, self.scan_code, self.flags)
    }

    pub const fn phase(&self) -> KeyPhase {
        if self.is_down() {
            KeyPhase::Down
        } else {
            KeyPhase::Up
        }
    }
}

/// A synthetic output key identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct OutputKey {
    virtual_key: u16,
    scan_code: u16,
    extended: bool,
}

impl OutputKey {
    pub const fn new(virtual_key: u16, scan_code: u16, extended: bool) -> Self {
        Self {
            virtual_key,
            scan_code,
            extended,
        }
    }

    /// Construct an output that is identified only by virtual key.
    pub const fn virtual_key(virtual_key: u16) -> Self {
        Self::new(virtual_key, 0, false)
    }

    /// Construct an output with an explicit scan code and extended state.
    pub const fn extended(virtual_key: u16, scan_code: u16) -> Self {
        Self::new(virtual_key, scan_code, true)
    }

    pub const fn virtual_key_code(self) -> u16 {
        self.virtual_key
    }

    /// Short alias used by output adapters and tests.
    pub const fn vk(self) -> u16 {
        self.virtual_key_code()
    }

    pub const fn scan_code(self) -> u16 {
        self.scan_code
    }

    pub const fn is_extended(self) -> bool {
        self.extended
    }
}

/// A target key's action mapping.
///
/// A mapping may contain one output key or a chord.  The constructor removes
/// duplicate output keys so a malformed mapping cannot create an unbalanced
/// reference count.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActionMapping {
    outputs: Vec<OutputKey>,
}

impl ActionMapping {
    pub fn new(output: OutputKey) -> Self {
        Self {
            outputs: vec![output],
        }
    }

    pub fn from_outputs<I>(outputs: I) -> Self
    where
        I: IntoIterator<Item = OutputKey>,
    {
        let mut unique = Vec::new();
        for output in outputs {
            if !unique.contains(&output) {
                unique.push(output);
            }
        }
        Self { outputs: unique }
    }

    pub fn chord<I>(outputs: I) -> Self
    where
        I: IntoIterator<Item = OutputKey>,
    {
        Self::from_outputs(outputs)
    }

    pub fn outputs(&self) -> &[OutputKey] {
        &self.outputs
    }

    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }
}

impl From<OutputKey> for ActionMapping {
    fn from(value: OutputKey) -> Self {
        Self::new(value)
    }
}

impl From<u16> for ActionMapping {
    fn from(value: u16) -> Self {
        Self::new(OutputKey::virtual_key(value))
    }
}

impl From<u8> for ActionMapping {
    fn from(value: u8) -> Self {
        Self::from(u16::from(value))
    }
}

impl From<Vec<OutputKey>> for ActionMapping {
    fn from(value: Vec<OutputKey>) -> Self {
        Self::from_outputs(value)
    }
}

impl From<&[OutputKey]> for ActionMapping {
    fn from(value: &[OutputKey]) -> Self {
        Self::from_outputs(value.iter().copied())
    }
}

/// Direction of a synthetic key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyPhase {
    Down,
    Up,
}

/// One synthetic output event emitted by the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InjectedKeyEvent {
    key: OutputKey,
    phase: KeyPhase,
    marker: u64,
}

impl InjectedKeyEvent {
    pub const fn new(key: OutputKey, phase: KeyPhase, marker: u64) -> Self {
        Self { key, phase, marker }
    }

    pub const fn key(self) -> OutputKey {
        self.key
    }

    pub const fn phase(self) -> KeyPhase {
        self.phase
    }

    pub const fn marker(self) -> u64 {
        self.marker
    }
}

/// One output event for the platform adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelayEvent {
    /// Re-inject the original sample unchanged except for this relay's marker.
    Forwarded(RawKeyboardSample),
    /// Emit one mapped synthetic output edge with this relay's marker.
    Injected(InjectedKeyEvent),
}

/// Why the relay selected a fail-open forwarding path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FailOpenReason {
    /// The target sample has no configured action mapping.
    TargetUnmapped,
    /// A key-up arrived without a retained mapped key-down route.
    MissingKeyDown,
    /// The source was removed before its matching key-up arrived.
    DeviceUnavailable,
    /// The relay has been shut down and can no longer safely synthesize state.
    RelayInactive,
    /// A marker from another injector was observed; it is not safe to map it.
    ForeignInjected,
}

/// Classification used by the relay and by platform adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceClass {
    Target,
    Ordinary,
    Unavailable,
}

/// High-level outcome for one accepted sample or cleanup operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RelayDecision {
    /// An ordinary source was forwarded unchanged.
    PassThrough,
    /// A target source was handled by a configured mapping.
    Mapped,
    /// The relay's own marker was ignored to prevent a feedback loop.
    IgnoredSelfInjected,
    /// The relay forwarded the sample because state was not safe to map.
    FailOpen(FailOpenReason),
    /// A device-removal cleanup released any outputs held by that source.
    DeviceRemoved,
    /// Shutdown cleanup released all remaining outputs.
    Shutdown,
}

/// Result of feeding a sample or lifecycle event to [`KeyboardRelay`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayResult {
    decision: RelayDecision,
    events: Vec<RelayEvent>,
}

impl RelayResult {
    fn new(decision: RelayDecision, events: Vec<RelayEvent>) -> Self {
        Self { decision, events }
    }

    pub const fn decision(&self) -> RelayDecision {
        self.decision
    }

    pub fn events(&self) -> &[RelayEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<RelayEvent> {
        self.events
    }

    pub fn is_fail_open(&self) -> bool {
        matches!(self.decision, RelayDecision::FailOpen(_))
    }

    pub fn fail_open_reason(&self) -> Option<FailOpenReason> {
        match self.decision {
            RelayDecision::FailOpen(reason) => Some(reason),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
struct HeldInputKey {
    device_id: DeviceId,
    input_key: InputKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HeldRoute {
    /// Mapping snapshot captured on the key-down edge.
    outputs: Vec<OutputKey>,
}

/// Platform-neutral all-keyboard relay state machine.
#[derive(Clone, Debug)]
pub struct KeyboardRelay {
    target_devices: BTreeSet<DeviceId>,
    unavailable_devices: BTreeSet<DeviceId>,
    mappings: BTreeMap<InputKey, ActionMapping>,
    held_inputs: BTreeMap<HeldInputKey, HeldRoute>,
    /// Forwarded physical keys that have not received their matching release
    /// yet.  `RIDEV_NOLEGACY` consumes the original edge, so a device-removal
    /// or shutdown boundary must synthesize the missing releases as well as
    /// releasing mapped target outputs.
    held_forwarded: BTreeMap<HeldInputKey, RawKeyboardSample>,
    held_outputs: BTreeMap<OutputKey, usize>,
    injection_marker: u64,
    active: bool,
}

impl Default for KeyboardRelay {
    fn default() -> Self {
        Self::new(std::iter::empty::<DeviceId>())
    }
}

impl KeyboardRelay {
    /// Construct a running relay for the supplied target source identities.
    pub fn new<I, D>(target_devices: I) -> Self
    where
        I: IntoIterator<Item = D>,
        D: Into<DeviceId>,
    {
        Self::with_marker(target_devices, DEFAULT_INJECTION_MARKER)
    }

    /// Construct a relay with an adapter-selected self-injection marker.
    pub fn with_marker<I, D>(target_devices: I, injection_marker: u64) -> Self
    where
        I: IntoIterator<Item = D>,
        D: Into<DeviceId>,
    {
        Self {
            target_devices: target_devices.into_iter().map(Into::into).collect(),
            unavailable_devices: BTreeSet::new(),
            mappings: BTreeMap::new(),
            held_inputs: BTreeMap::new(),
            held_forwarded: BTreeMap::new(),
            held_outputs: BTreeMap::new(),
            injection_marker,
            active: true,
        }
    }

    pub const fn injection_marker(&self) -> u64 {
        self.injection_marker
    }

    pub const fn is_active(&self) -> bool {
        self.active
    }

    pub const fn is_shutdown(&self) -> bool {
        !self.active
    }

    pub fn target_devices(&self) -> impl Iterator<Item = &DeviceId> {
        self.target_devices.iter()
    }

    /// Classify a source using the current target and removal sets.
    pub fn classify_device(&self, device_id: impl Into<DeviceId>) -> DeviceClass {
        let device_id = device_id.into();
        if self.unavailable_devices.contains(&device_id) {
            DeviceClass::Unavailable
        } else if self.target_devices.contains(&device_id) {
            DeviceClass::Target
        } else {
            DeviceClass::Ordinary
        }
    }

    /// Add or remove a target source from the configured target set.
    ///
    /// Existing held routes are intentionally retained until release or
    /// cleanup; this prevents a mapping change from leaving a synthetic key
    /// held under a different route.
    pub fn set_target_device(&mut self, device_id: impl Into<DeviceId>, target: bool) {
        let device_id = device_id.into();
        if target {
            self.target_devices.insert(device_id.clone());
            self.unavailable_devices.remove(&device_id);
        } else {
            self.target_devices.remove(&device_id);
            self.unavailable_devices.insert(device_id);
        }
    }

    pub fn add_target_device(&mut self, device_id: impl Into<DeviceId>) {
        self.set_target_device(device_id, true);
    }

    pub fn remove_target_device(&mut self, device_id: impl Into<DeviceId>) {
        self.set_target_device(device_id, false);
    }

    /// Mark a previously removed source as available again.
    pub fn mark_device_available(&mut self, device_id: impl Into<DeviceId>) {
        self.unavailable_devices.remove(&device_id.into());
    }

    /// Record a virtual-key mapping that applies regardless of scan code and
    /// extended state.  The mapping is cloned into each target key-down route.
    pub fn set_mapping<M>(&mut self, virtual_key: u16, mapping: M)
    where
        M: Into<ActionMapping>,
    {
        self.set_key_mapping(InputKey::virtual_key(virtual_key), mapping);
    }

    /// Alias for profile-oriented call sites.
    pub fn map_key<M>(&mut self, virtual_key: u16, mapping: M)
    where
        M: Into<ActionMapping>,
    {
        self.set_mapping(virtual_key, mapping);
    }

    /// Record an exact virtual-key/scan-code/extended-state mapping.
    pub fn set_key_mapping<M>(&mut self, input_key: InputKey, mapping: M)
    where
        M: Into<ActionMapping>,
    {
        let mapping = mapping.into();
        if mapping.is_empty() {
            self.mappings.remove(&input_key);
        } else {
            self.mappings.insert(input_key, mapping);
        }
    }

    pub fn remove_mapping(&mut self, virtual_key: u16) -> Option<ActionMapping> {
        self.mappings.remove(&InputKey::virtual_key(virtual_key))
    }

    pub fn remove_key_mapping(&mut self, input_key: InputKey) -> Option<ActionMapping> {
        self.mappings.remove(&input_key)
    }

    /// Resolve an exact mapping first, then a virtual-key-wide mapping.
    pub fn mapping_for(&self, input_key: InputKey) -> Option<&ActionMapping> {
        self.mappings.get(&input_key).or_else(|| {
            self.mappings
                .get(&InputKey::virtual_key(input_key.virtual_key_code()))
        })
    }

    pub fn mapping_for_virtual_key(&self, virtual_key: u16) -> Option<&ActionMapping> {
        self.mappings.get(&InputKey::virtual_key(virtual_key))
    }

    /// Feed one Raw Input sample into the state machine.
    pub fn handle(&mut self, sample: RawKeyboardSample) -> RelayResult {
        if sample.injection_marker() == Some(self.injection_marker) {
            return RelayResult::new(RelayDecision::IgnoredSelfInjected, Vec::new());
        }

        if sample.injection_marker().is_some() {
            let event = RelayEvent::Forwarded(self.forward_sample(sample));
            return RelayResult::new(
                RelayDecision::FailOpen(FailOpenReason::ForeignInjected),
                vec![event],
            );
        }

        if !self.active {
            let event = RelayEvent::Forwarded(self.forward_sample(sample));
            return RelayResult::new(
                RelayDecision::FailOpen(FailOpenReason::RelayInactive),
                vec![event],
            );
        }

        if sample.is_down() {
            self.handle_down(sample)
        } else {
            self.handle_up(sample)
        }
    }

    /// Synonym for event-loop call sites.
    pub fn process(&mut self, sample: RawKeyboardSample) -> RelayResult {
        self.handle(sample)
    }

    /// Synonym for adapters that call the relay an input consumer.
    pub fn accept(&mut self, sample: RawKeyboardSample) -> RelayResult {
        self.handle(sample)
    }

    /// Mark one source unavailable and release every output it still holds.
    pub fn remove_device(&mut self, device_id: impl Into<DeviceId>) -> RelayResult {
        let device_id = device_id.into();
        self.unavailable_devices.insert(device_id.clone());
        let events = self.release_device(&device_id);
        RelayResult::new(RelayDecision::DeviceRemoved, events)
    }

    /// Release all synthetic state and stop the relay.
    pub fn shutdown(&mut self) -> RelayResult {
        self.active = false;
        let events = self.release_all();
        RelayResult::new(RelayDecision::Shutdown, events)
    }

    /// Re-enable a relay after a completed shutdown.  Mappings and target
    /// identities remain configured, while all held state is guaranteed empty.
    pub fn restart(&mut self) {
        self.held_inputs.clear();
        self.held_forwarded.clear();
        self.held_outputs.clear();
        self.active = true;
    }

    /// Alias for lifecycle code that calls this operation `start`.
    pub fn start(&mut self) {
        self.restart();
    }

    /// Reset synthetic state and leave the relay inactive.
    pub fn reset(&mut self) -> RelayResult {
        self.shutdown()
    }

    pub fn held_input_count(&self) -> usize {
        self.held_inputs.len()
    }

    /// Number of physical source keys currently represented by forwarded
    /// output records.  This is primarily useful to platform cleanup and
    /// acceptance telemetry; mapped target routes are reported separately by
    /// [`Self::held_input_count`].
    pub fn held_forwarded_count(&self) -> usize {
        self.held_forwarded.len()
    }

    /// Number of distinct output keys currently held.
    pub fn held_output_count(&self) -> usize {
        self.held_outputs.len()
    }

    /// Total number of source routes holding an output key.
    pub fn output_ref_count(&self, output: OutputKey) -> usize {
        self.held_outputs.get(&output).copied().unwrap_or(0)
    }

    pub fn is_input_held(&self, device_id: impl Into<DeviceId>, input_key: InputKey) -> bool {
        self.held_inputs.contains_key(&HeldInputKey {
            device_id: device_id.into(),
            input_key,
        })
    }

    fn handle_down(&mut self, sample: RawKeyboardSample) -> RelayResult {
        let device_id = sample.device_id.clone();
        match self.classify_device(device_id.clone()) {
            DeviceClass::Ordinary => {
                let event = self.forwarded_event(sample);
                RelayResult::new(RelayDecision::PassThrough, vec![event])
            }
            DeviceClass::Unavailable => {
                let event = RelayEvent::Forwarded(self.forward_sample(sample));
                RelayResult::new(
                    RelayDecision::FailOpen(FailOpenReason::DeviceUnavailable),
                    vec![event],
                )
            }
            DeviceClass::Target => {
                let input_key = sample.input_key();
                let held_key = HeldInputKey {
                    device_id,
                    input_key,
                };

                // Typematic repeats do not create another logical holder;
                // otherwise one physical key-up could never balance them.
                if self.held_inputs.contains_key(&held_key) {
                    return RelayResult::new(RelayDecision::Mapped, Vec::new());
                }

                let Some(mapping) = self.mapping_for(input_key).cloned() else {
                    let event = self.forwarded_event(sample);
                    return RelayResult::new(
                        RelayDecision::FailOpen(FailOpenReason::TargetUnmapped),
                        vec![event],
                    );
                };

                let mut events = Vec::new();
                for output in mapping.outputs().iter().copied() {
                    let count = self.held_outputs.entry(output).or_default();
                    if *count == 0 {
                        events.push(RelayEvent::Injected(InjectedKeyEvent::new(
                            output,
                            KeyPhase::Down,
                            self.injection_marker,
                        )));
                    }
                    *count += 1;
                }
                self.held_inputs.insert(
                    held_key,
                    HeldRoute {
                        outputs: mapping.outputs().to_vec(),
                    },
                );
                RelayResult::new(RelayDecision::Mapped, events)
            }
        }
    }

    fn handle_up(&mut self, sample: RawKeyboardSample) -> RelayResult {
        let device_id = sample.device_id.clone();
        let held_key = HeldInputKey {
            device_id: device_id.clone(),
            input_key: sample.input_key(),
        };

        // Route lookup precedes current device classification.  A target may
        // have been reclassified or removed after its key-down; its snapshot
        // is still the only safe source of the matching release.
        if let Some(route) = self.held_inputs.remove(&held_key) {
            let events = self.release_route(route);
            return RelayResult::new(RelayDecision::Mapped, events);
        }

        let reason = match self.classify_device(device_id) {
            DeviceClass::Ordinary => None,
            DeviceClass::Unavailable => Some(FailOpenReason::DeviceUnavailable),
            DeviceClass::Target => Some(FailOpenReason::MissingKeyDown),
        };

        let event = self.forwarded_event(sample);
        match reason {
            Some(reason) => RelayResult::new(RelayDecision::FailOpen(reason), vec![event]),
            None => RelayResult::new(RelayDecision::PassThrough, vec![event]),
        }
    }

    fn forward_sample(&self, mut sample: RawKeyboardSample) -> RawKeyboardSample {
        sample.injection_marker = Some(self.injection_marker);
        sample
    }

    fn forwarded_event(&mut self, sample: RawKeyboardSample) -> RelayEvent {
        let held_key = HeldInputKey {
            device_id: sample.device_id.clone(),
            input_key: sample.input_key(),
        };
        if sample.injection_marker.is_none() {
            if sample.is_down() {
                // Keep the first physical down as the identity source for a
                // later cleanup release; typematic repeats must not replace
                // it or create additional held records.
                self.held_forwarded
                    .entry(held_key)
                    .or_insert_with(|| sample.clone());
            } else {
                self.held_forwarded.remove(&held_key);
            }
        }
        RelayEvent::Forwarded(self.forward_sample(sample))
    }

    fn release_route(&mut self, route: HeldRoute) -> Vec<RelayEvent> {
        let mut events = Vec::new();
        for output in route.outputs.into_iter().rev() {
            let Some(count) = self.held_outputs.get_mut(&output) else {
                // The cleanup methods preserve this invariant, but a safe
                // no-op keeps a malformed/stale route fail-open rather than
                // underflowing a reference count.
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.held_outputs.remove(&output);
                events.push(RelayEvent::Injected(InjectedKeyEvent::new(
                    output,
                    KeyPhase::Up,
                    self.injection_marker,
                )));
            }
        }
        events
    }

    fn release_device(&mut self, device_id: &DeviceId) -> Vec<RelayEvent> {
        let keys: Vec<HeldInputKey> = self
            .held_inputs
            .keys()
            .filter(|key| &key.device_id == device_id)
            .cloned()
            .collect();
        let mut events = Vec::new();
        for key in keys {
            if let Some(route) = self.held_inputs.remove(&key) {
                events.extend(self.release_route(route));
            }
        }
        let forwarded_keys: Vec<HeldInputKey> = self
            .held_forwarded
            .keys()
            .filter(|key| &key.device_id == device_id)
            .cloned()
            .collect();
        for key in forwarded_keys {
            if let Some(sample) = self.held_forwarded.remove(&key) {
                events.push(self.forwarded_release(sample));
            }
        }
        events
    }

    fn release_all(&mut self) -> Vec<RelayEvent> {
        let mut events = Vec::new();
        for output in self.held_outputs.keys().copied().collect::<Vec<_>>() {
            events.push(RelayEvent::Injected(InjectedKeyEvent::new(
                output,
                KeyPhase::Up,
                self.injection_marker,
            )));
        }
        for sample in self.held_forwarded.values().cloned() {
            events.push(self.forwarded_release(sample));
        }
        self.held_inputs.clear();
        self.held_forwarded.clear();
        self.held_outputs.clear();
        events
    }

    fn forwarded_release(&self, mut sample: RawKeyboardSample) -> RelayEvent {
        sample.flags |= RAW_KEY_BREAK;
        RelayEvent::Forwarded(self.forward_sample(sample))
    }
}

/// Naming aliases for adapters that use Raw Input terminology explicitly.
pub type RawInputKeyboardSample = RawKeyboardSample;
pub type KeyboardRelayCore = KeyboardRelay;
pub type RelayMapping = ActionMapping;
pub type RelayOutputKey = OutputKey;
