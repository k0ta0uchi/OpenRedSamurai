//! Profile-to-device application planning.
//!
//! This module is deliberately conservative.  `ANALYSIS.md` identifies the
//! wire shape and a status-query pair, but it does not establish a verified
//! profile-field-to-command mapping.  Consequently a plan records the
//! observed status query separately, reports every profile value that would
//! need a mapping, and never invents a setting or commit frame.  A status
//! query is not part of the write list: keeping it separate prevents an
//! informational/read-only observation from being sent through the apply
//! transport by accident.

use crate::device_protocol::{
    command_info, ApplySequenceError, CommandCapability, DpiSelectionReadback, FrameError,
    PollingRateReadback, ReportFrame, VerifiedApplySequence, VerifiedWrite, CMD_F2, PARAM_LEN,
    REPORT_ID_CONFIG, SUBCMD_F2_STATUS,
};
use crate::profile::Profile;
use std::error::Error;
use std::fmt;

/// The phases used to order frames in an application plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyPhase {
    /// Read-only status/query traffic observed as `02/F2/2C`.  This phase is
    /// retained for callers that inspect protocol phases; the query itself is
    /// stored separately from the write frames.
    StatusQuery,
    /// Profile setting writes.  No such frame is emitted until a field
    /// mapping is verified in the protocol table.
    Settings,
    /// Device commit/persist traffic.  No commit frame is emitted while its
    /// parameter layout/effect remains unverified.
    Commit,
}

/// A non-fatal reason a profile value was not included as a device frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyWarning {
    /// Profile key or logical field that could not be mapped safely.
    pub field: String,
    /// Stable, human-readable explanation suitable for a dry-run UI.
    pub detail: String,
}

impl ApplyWarning {
    pub fn new(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            detail: detail.into(),
        }
    }

    /// Borrow the logical field name without exposing representation details.
    pub fn field_name(&self) -> &str {
        &self.field
    }

    /// Borrow the warning explanation.
    pub fn reason(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for ApplyWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.detail)
    }
}

/// Controls which observed protocol phases are included in a dry-run plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyOptions {
    /// Retain the observed status query alongside the plan.  It is not added
    /// to the write frame list.
    pub include_status_query: bool,
    /// Request a commit phase.  This currently produces a warning because no
    /// commit command has a verified parameter mapping.
    pub include_commit: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            include_status_query: true,
            include_commit: false,
        }
    }
}

/// Ordered, dry-run representation of profile application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyPlan {
    /// Device-protocol frames retained for dry-run inspection.  Raw frames
    /// never carry write authority and are rejected by every legacy apply
    /// method, even if a future protocol-table entry becomes writable.
    pub frames: Vec<ReportFrame>,
    /// Evidence-backed write tokens.  This is private so callers cannot turn
    /// a raw `ReportFrame` into a write through a struct literal.
    verified_frames: Vec<VerifiedWrite>,
    /// Evidence-backed ordered Apply sequence.  Unlike [`Self::frames`], this
    /// field is a protocol authorization token and may only be consumed by
    /// the sequence-gated apply APIs below.
    verified_sequence: Option<VerifiedApplySequence>,
    /// The observed status query, retained for inspection but never included
    /// in [`Self::frames`] or sent by the write-oriented apply methods.
    status_query: Option<ReportFrame>,
    /// Unsupported or unverified profile fields; no guessed frame is emitted
    /// for any warning.
    pub warnings: Vec<ApplyWarning>,
    phases: Vec<ApplyPhase>,
}

impl Default for ApplyPlan {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            verified_frames: Vec::new(),
            verified_sequence: None,
            status_query: None,
            warnings: Vec::new(),
            phases: Vec::new(),
        }
    }
}

/// Whether device-side persistence was observed for an Apply operation.
///
/// The current Apply path does not issue a guessed readback request and does
/// not have a Rust-owned USBPcap capture.  Keeping the only current state
/// explicit prevents a successful transfer from being mistaken for a
/// persistence proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyPersistence {
    /// No current-binary readback observation is attached to this result.
    NotObserved,
}

impl ApplyPersistence {
    /// Whether this result contains a verified device-side persistence proof.
    pub const fn is_verified(self) -> bool {
        false
    }

    /// Stable text for bounded live-probe evidence lines.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotObserved => "not-observed",
        }
    }
}

/// Transfer-level record returned after an explicit Apply operation.
///
/// `expected_report_lengths` describes the exact lengths derived from the
/// already-authorized token or encoded frames.  It is not a USBPcap capture;
/// the separate [`ApplyPersistence`] state remains fail-closed until a
/// current-binary readback/capture artifact is attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyTransferRecord {
    /// Number of reports expected by the authorized plan.
    pub expected_reports: usize,
    /// Number of complete reports acknowledged by the transport.
    pub completed_reports: usize,
    /// Expected report lengths in transfer order.
    pub expected_report_lengths: Vec<usize>,
}

impl ApplyTransferRecord {
    /// True only when the transport acknowledged every expected report and
    /// the result retains one expected length for each report.
    pub fn is_complete(&self) -> bool {
        self.expected_reports == self.completed_reports
            && self.expected_reports == self.expected_report_lengths.len()
    }
}

/// Result returned after an explicit transport-backed apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    /// Number of complete feature reports accepted by the transport.
    pub sent: usize,
    /// Warnings carried by the plan.  A plan with warnings is rejected before
    /// transport I/O, so this is normally empty for a successful apply.
    pub warnings: Vec<ApplyWarning>,
    /// Explicit transfer/completion record for current-binary diagnostics.
    pub transfer: ApplyTransferRecord,
    /// Device-side persistence remains unobserved until a readback/capture is
    /// explicitly supplied by a future evidence harness.
    pub persistence: ApplyPersistence,
}

/// Generic transport boundary consumed by [`ApplyPlan::apply`].
///
/// Implementations receive a validated protocol frame rather than a guessed
/// field structure. This keeps planning independent of HID I/O and makes a
/// mock transport trivial to write in tests.
pub trait ApplyTransport {
    type Error: fmt::Display;

    fn send(&mut self, frame: &ReportFrame) -> Result<(), Self::Error>;
}

/// Byte-oriented transport boundary consumed by [`ApplyPlan::apply_to`].
///
/// The transport receives already encoded feature-report bytes and returns
/// the number of bytes accepted.  This matches the HID API seam while making
/// planning and unit tests independent of actual HID I/O.
pub trait FeatureReportTransport {
    type Error: fmt::Display;

    fn send_feature_report(&mut self, bytes: &[u8]) -> Result<usize, Self::Error>;
}

/// HID transport boundary for evidence-backed writes.
///
/// This is intentionally separate from [`FeatureReportTransport`].  A
/// hardware adapter must explicitly consume a [`VerifiedWrite`] token rather
/// than accepting arbitrary bytes from a plan.
pub trait VerifiedFeatureReportTransport {
    type Error: fmt::Display;

    fn send_verified_write(&mut self, write: &VerifiedWrite) -> Result<usize, Self::Error>;
}

/// Frame-oriented transport boundary for evidence-backed writes.
pub trait VerifiedApplyTransport {
    type Error: fmt::Display;

    fn send_verified_write(&mut self, write: &VerifiedWrite) -> Result<(), Self::Error>;
}

/// Transport boundary for the complete evidence-backed Apply sequence.
///
/// The sequence is passed as one authorization token. Implementations must
/// preserve its order and report count; callers cannot submit an arbitrary
/// `ReportFrame` through this seam.
pub trait VerifiedSequenceTransport {
    type Error: fmt::Display;

    fn send_verified_sequence(
        &mut self,
        sequence: &VerifiedApplySequence,
    ) -> Result<usize, Self::Error>;
}

/// Read-only observation boundary for the exact PollingRate response that the
/// official post-Apply route returned.  Implementations must not construct or
/// send a command frame; the result is an observation, not write authority.
pub trait PollingRateReadbackTransport {
    type Error: fmt::Display;

    fn observe_polling_rate_readback(&mut self) -> Result<PollingRateReadback, Self::Error>;
}

/// Read-only observation boundary for the selected-DPI response proven by the
/// controlled Run A/Run B reconnect/readback capture.  Implementations must
/// not construct or send a command frame.
pub trait DpiSelectionReadbackTransport {
    type Error: fmt::Display;

    fn observe_dpi_selection_readback(&mut self) -> Result<DpiSelectionReadback, Self::Error>;
}

/// Adapt the concrete HID transport to the planning seam.  The device layer
/// returns `()` after accepting a report, so the adapter reports the exact
/// number of bytes that were handed to it.  Passing the encoded length back to
/// the device layer preserves the observed 16/64/1024-byte header binding.
impl FeatureReportTransport for crate::device::DeviceTransport {
    type Error = crate::device::DeviceError;

    fn send_feature_report(&mut self, bytes: &[u8]) -> Result<usize, Self::Error> {
        crate::device::DeviceTransport::send_feature_report_exact(self, bytes, bytes.len())?;
        Ok(bytes.len())
    }
}

impl VerifiedFeatureReportTransport for crate::device::DeviceTransport {
    type Error = crate::device::DeviceError;

    fn send_verified_write(&mut self, write: &VerifiedWrite) -> Result<usize, Self::Error> {
        let bytes = write
            .as_bytes()
            .map_err(|error| crate::device::DeviceError::Io {
                operation: "encode verified feature report",
                message: error.to_string(),
            })?;
        crate::device::DeviceTransport::send_verified_write(self, write)?;
        Ok(bytes.len())
    }
}

/// Adapt the concrete HID transport to the frame-oriented apply seam.
impl ApplyTransport for crate::device::DeviceTransport {
    type Error = crate::device::DeviceError;

    fn send(&mut self, frame: &ReportFrame) -> Result<(), Self::Error> {
        let bytes = frame
            .encode()
            .map_err(|error| crate::device::DeviceError::Io {
                operation: "encode feature report",
                message: error.to_string(),
            })?;
        crate::device::DeviceTransport::send_feature_report_exact(self, &bytes, bytes.len())
    }
}

impl VerifiedApplyTransport for crate::device::DeviceTransport {
    type Error = crate::device::DeviceError;

    fn send_verified_write(&mut self, write: &VerifiedWrite) -> Result<(), Self::Error> {
        crate::device::DeviceTransport::send_verified_write(self, write)
    }
}

impl VerifiedSequenceTransport for crate::device::DeviceTransport {
    type Error = crate::device::DeviceError;

    fn send_verified_sequence(
        &mut self,
        sequence: &VerifiedApplySequence,
    ) -> Result<usize, Self::Error> {
        crate::device::DeviceTransport::send_verified_apply_sequence(self, sequence)
    }
}

impl PollingRateReadbackTransport for crate::device::DeviceTransport {
    type Error = crate::device::DeviceError;

    fn observe_polling_rate_readback(&mut self) -> Result<PollingRateReadback, Self::Error> {
        crate::device::DeviceTransport::observe_polling_rate_readback(self)
    }
}

impl DpiSelectionReadbackTransport for crate::device::DeviceTransport {
    type Error = crate::device::DeviceError;

    fn observe_dpi_selection_readback(&mut self) -> Result<DpiSelectionReadback, Self::Error> {
        crate::device::DeviceTransport::observe_dpi_selection_readback(self)
    }
}

/// Errors that stop an explicit apply before an unsafe or incomplete write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// The fixed observed status frame could not be built or validated.
    Protocol(FrameError),
    /// The protocol table no longer marks the status header as read-only.
    StatusQueryUnavailable,
    /// The plan contains fields that have no verified device mapping.
    UnsupportedFields { warnings: Vec<ApplyWarning> },
    /// A frame outside the observed/read-only set was present in a plan.
    UnsupportedFrame {
        index: usize,
        report_id: u8,
        cmd: u8,
        subcmd: u8,
    },
    /// A token was constructed from a mapping that is not present in the
    /// reviewed registry.  Test fixtures and stale mappings must not reach a
    /// transport even when their wire shape is otherwise valid.
    UnverifiedWrite {
        index: usize,
        report_id: u8,
        cmd: u8,
        subcmd: u8,
    },
    /// The transport rejected one of the explicitly requested frames.
    Transport { index: usize, detail: String },
    /// A transport accepted fewer bytes than the exact encoded frame length.
    InvalidTransportLength {
        index: usize,
        actual: usize,
        expected: usize,
    },
    /// A plan contains evidence-backed tokens but was passed to the legacy
    /// raw-frame apply method.  Use an explicit `apply_verified_*` method so
    /// the token reaches the transport boundary intact.
    VerifiedWritesRequireGatedApply,
    /// A complete ordered Apply sequence requires its explicit sequence-gated
    /// transport method rather than any raw-frame entry point.
    VerifiedSequenceRequiresGatedApply,
    /// A sequence transport must acknowledge every complete report.
    InvalidSequenceCompletion { actual: usize, expected: usize },
    /// The read-only PollingRate observation failed after an authorized
    /// sequence transfer.
    ReadbackTransport { detail: String },
    /// The read-only response did not retain the wire value authorized by the
    /// complete Apply sequence.
    ReadbackMismatch { actual: u8, expected: u8 },
    /// The protocol authorization token failed validation before transport.
    InvalidVerifiedSequence(ApplySequenceError),
    /// No complete ordered authorization token was attached to the plan.
    MissingVerifiedSequence,
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(f, "protocol frame error: {error}"),
            Self::StatusQueryUnavailable => write!(f, "observed status query is unavailable"),
            Self::UnsupportedFields { warnings } => {
                write!(f, "{} unsupported profile field(s)", warnings.len())
            }
            Self::UnsupportedFrame {
                index,
                report_id,
                cmd,
                subcmd,
            } => write!(
                f,
                "unsupported frame {index}: {report_id:02X}/{cmd:02X}/{subcmd:02X}"
            ),
            Self::UnverifiedWrite {
                index,
                report_id,
                cmd,
                subcmd,
            } => write!(
                f,
                "unregistered verified write {index}: {report_id:02X}/{cmd:02X}/{subcmd:02X}"
            ),
            Self::Transport { index, detail } => {
                write!(f, "transport error at frame {index}: {detail}")
            }
            Self::InvalidTransportLength {
                index,
                actual,
                expected,
            } => write!(
                f,
                "transport accepted {actual} bytes at frame {index}; expected {expected}"
            ),
            Self::VerifiedWritesRequireGatedApply => write!(
                f,
                "verified writes require the explicit token-gated apply method"
            ),
            Self::VerifiedSequenceRequiresGatedApply => write!(
                f,
                "verified Apply sequences require the explicit sequence-gated apply method"
            ),
            Self::InvalidSequenceCompletion { actual, expected } => write!(
                f,
                "verified Apply sequence completed {actual} reports; expected {expected}"
            ),
            Self::ReadbackTransport { detail } => {
                write!(f, "PollingRate readback observation failed: {detail}")
            }
            Self::ReadbackMismatch { actual, expected } => write!(
                f,
                "PollingRate readback wire value 0x{actual:02X} does not match authorized value 0x{expected:02X}"
            ),
            Self::InvalidVerifiedSequence(error) => {
                write!(f, "invalid verified Apply sequence: {error}")
            }
            Self::MissingVerifiedSequence => {
                write!(f, "no verified Apply sequence is attached to this plan")
            }
        }
    }
}

impl Error for ApplyError {}

impl From<FrameError> for ApplyError {
    fn from(value: FrameError) -> Self {
        Self::Protocol(value)
    }
}

const STATUS_QUERY_PARAMS: [u8; PARAM_LEN] = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// Build the exact status/query frame observed in `ANALYSIS.md`.
pub fn status_query_frame() -> Result<ReportFrame, ApplyError> {
    let Some(info) = command_info(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_STATUS) else {
        return Err(ApplyError::StatusQueryUnavailable);
    };
    if info.capability != CommandCapability::ReadOnly {
        return Err(ApplyError::StatusQueryUnavailable);
    }
    ReportFrame::new(
        REPORT_ID_CONFIG,
        CMD_F2,
        SUBCMD_F2_STATUS,
        STATUS_QUERY_PARAMS,
    )
    .map_err(ApplyError::Protocol)
}

/// Alias for callers that use a verb rather than a noun.
pub fn build_status_query() -> Result<ReportFrame, ApplyError> {
    status_query_frame()
}

impl ApplyPlan {
    /// Create a write-ready plan from an already-authorized complete Apply
    /// sequence.
    ///
    /// This constructor is used by the Rust-owned reconnect worker.  It does
    /// not inspect or emit arbitrary profile fields; the supplied sequence is
    /// revalidated against the reviewed 156-report token before it can reach
    /// a transport.
    pub fn from_verified_sequence(sequence: VerifiedApplySequence) -> Result<Self, ApplyError> {
        let sequence = VerifiedApplySequence::try_from_frames(sequence.frames())
            .map_err(ApplyError::InvalidVerifiedSequence)?;
        Ok(Self {
            verified_sequence: Some(sequence),
            ..Self::default()
        })
    }

    /// Create the default dry-run plan.  It records the observed status query
    /// separately and no commit request; unsupported fields are represented
    /// only as warnings and never as speculative frames.
    pub fn dry_run(profile: &Profile) -> Self {
        Self::try_build(profile).unwrap_or_else(|error| {
            let mut plan = Self::default();
            plan.warnings
                .push(ApplyWarning::new("Protocol", error.to_string()));
            plan
        })
    }

    /// Build a plan while retaining a typed error for protocol-table failures.
    pub fn try_build(profile: &Profile) -> Result<Self, ApplyError> {
        Self::try_build_with_options(profile, ApplyOptions::default())
    }

    /// Build an explicitly authorized profile plan.
    ///
    /// `dry_run`/`try_build` intentionally remain conservative and continue to
    /// report `PollingRate` and unsupported DPI selections as warnings. This
    /// constructor is the only profile-to-write path that asks the protocol
    /// gate for its complete reviewed Apply sequence. A sequence is attached
    /// only for the authorized PollingRate and (when present) the two
    /// reconnect/readback-proven DPI selection values.
    pub fn try_build_authorized(profile: &Profile) -> Result<Self, ApplyError> {
        Self::try_build_authorized_with_options(profile, ApplyOptions::default())
    }

    /// Alias emphasizing that this constructor may produce a write-ready
    /// authorization token while ordinary builders remain dry-run-only.
    pub fn authorized(profile: &Profile) -> Result<Self, ApplyError> {
        Self::try_build_authorized(profile)
    }

    /// Fallible alias for callers that prefer the profile terminology.
    pub fn try_from_profile_authorized(profile: &Profile) -> Result<Self, ApplyError> {
        Self::try_build_authorized(profile)
    }

    /// Build a plan with explicit phase options.
    pub fn try_build_with_options(
        profile: &Profile,
        options: ApplyOptions,
    ) -> Result<Self, ApplyError> {
        let mut plan = Self::default();
        collect_profile_warnings(profile, &mut plan.warnings);

        if options.include_status_query {
            plan.status_query = Some(status_query_frame()?);
        }

        if options.include_commit {
            // F1/F5 commit/reset-family headers are present in the observed
            // table, but their pairing, parameters, and effect are unresolved.
            plan.warnings.push(ApplyWarning::new(
                "Commit",
                "F1/F5 commit mapping is observed but unverified; no frame emitted",
            ));
        }

        Ok(plan)
    }

    /// Build an explicitly authorized profile plan with phase options.
    pub fn try_build_authorized_with_options(
        profile: &Profile,
        options: ApplyOptions,
    ) -> Result<Self, ApplyError> {
        let mut plan = Self::default();
        let sequence = authorized_polling_sequence(profile)?;

        // Preserve all existing warning behavior. The PollingRate warning is
        // omitted when the protocol gate accepted and attached the complete
        // sequence. A DPI warning is omitted only when the attached sequence
        // carries one of the two A/B-proven selected values.
        collect_profile_warnings_with_polling(profile, &mut plan.warnings, sequence.is_none());
        if sequence
            .as_ref()
            .and_then(VerifiedApplySequence::profile_dpi_selection)
            .is_some()
        {
            plan.warnings.retain(|warning| warning.field != "DPI");
        }
        plan.verified_sequence = sequence;

        if options.include_status_query {
            plan.status_query = Some(status_query_frame()?);
        }

        if options.include_commit {
            plan.warnings.push(ApplyWarning::new(
                "Commit",
                "F1/F5 commit mapping is observed but unverified; no frame emitted",
            ));
        }

        Ok(plan)
    }

    /// Build a plan with default options (alias for [`Self::dry_run`]).
    pub fn from_profile(profile: &Profile) -> Self {
        Self::dry_run(profile)
    }

    /// Fallible alias for callers that need to surface protocol errors.
    pub fn try_from_profile(profile: &Profile) -> Result<Self, ApplyError> {
        Self::try_build(profile)
    }

    /// Fallible profile builder with explicit phase options.
    pub fn try_from_profile_with_options(
        profile: &Profile,
        options: ApplyOptions,
    ) -> Result<Self, ApplyError> {
        Self::try_build_with_options(profile, options)
    }

    /// Return the phase associated with one ordered frame.
    pub fn phase(&self, index: usize) -> Option<ApplyPhase> {
        self.phases.get(index).copied()
    }

    /// Return all frame phases in frame order.
    pub fn phases(&self) -> &[ApplyPhase] {
        &self.phases
    }

    /// Borrow the observed status query, if the builder was asked to retain
    /// it.  The returned frame is informational and is intentionally not part
    /// of the write-oriented apply methods.
    pub fn status_query(&self) -> Option<&ReportFrame> {
        self.status_query.as_ref()
    }

    /// Borrow evidence-backed write tokens attached to this plan.
    pub fn verified_frames(&self) -> &[VerifiedWrite] {
        &self.verified_frames
    }

    /// Borrow the complete ordered Apply authorization, if this plan was
    /// built through [`Self::try_build_authorized`].
    pub fn verified_sequence(&self) -> Option<&VerifiedApplySequence> {
        self.verified_sequence.as_ref()
    }

    /// Attach evidence-backed write tokens to a plan.
    ///
    /// The normal profile builder leaves this empty until a complete field
    /// mapping and ordered command sequence have been reviewed.  The method
    /// is public so a future evidence-backed builder can opt into the gated
    /// path without exposing mutable access to the token list.
    pub fn with_verified_writes<I>(mut self, writes: I) -> Self
    where
        I: IntoIterator<Item = VerifiedWrite>,
    {
        self.verified_frames = writes.into_iter().collect();
        self
    }

    /// Attach evidence-backed write tokens while rejecting any raw frames
    /// already present in the plan.  This fallible variant lets a builder
    /// enforce the token-only invariant at construction time instead of
    /// waiting for an apply call to fail.
    pub fn try_with_verified_writes<I>(mut self, writes: I) -> Result<Self, ApplyError>
    where
        I: IntoIterator<Item = VerifiedWrite>,
    {
        if self.verified_sequence.is_some() {
            return Err(ApplyError::VerifiedSequenceRequiresGatedApply);
        }
        if let Some((index, frame)) = self.frames.iter().enumerate().next() {
            return Err(ApplyError::UnsupportedFrame {
                index,
                report_id: frame.report_id,
                cmd: frame.cmd,
                subcmd: frame.subcmd,
            });
        }
        self.verified_frames = writes.into_iter().collect();
        Ok(self)
    }

    /// Whether the plan has a complete, warning-free token sequence ready for
    /// the gated transport adapter.  A dry-run with no profile changes is
    /// intentionally not write-ready: it has no verified token to send.
    pub fn is_write_ready(&self) -> bool {
        if !self.warnings.is_empty() || !self.frames.is_empty() {
            return false;
        }
        if self.verified_sequence.is_some() && !self.verified_frames.is_empty() {
            return false;
        }

        let legacy_ready = !self.verified_frames.is_empty()
            && self
                .verified_frames
                .iter()
                .all(VerifiedWrite::is_registry_authorized);
        let sequence_ready = self.verified_sequence.as_ref().is_some_and(|sequence| {
            VerifiedApplySequence::try_from_frames(sequence.frames()).is_ok()
        });

        legacy_ready || sequence_ready
    }

    /// Whether this plan contains no unsupported profile values.
    pub fn is_supported(&self) -> bool {
        self.warnings.is_empty()
    }

    /// Apply through a byte-oriented transport.  Unsupported fields fail
    /// closed before the first transport call.
    pub fn apply_to<T>(&self, transport: &mut T) -> Result<ApplyOutcome, ApplyError>
    where
        T: FeatureReportTransport,
    {
        if !self.warnings.is_empty() {
            return Err(ApplyError::UnsupportedFields {
                warnings: self.warnings.clone(),
            });
        }
        if !self.verified_frames.is_empty() {
            return Err(ApplyError::VerifiedWritesRequireGatedApply);
        }
        if self.verified_sequence.is_some() {
            return Err(ApplyError::VerifiedSequenceRequiresGatedApply);
        }

        let expected_report_lengths = self
            .frames
            .iter()
            .map(ReportFrame::expected_len)
            .collect::<Vec<_>>();
        let mut sent = 0;
        for (index, frame) in self.frames.iter().enumerate() {
            validate_sendable_frame(index, frame)?;
            let bytes = frame.encode().map_err(ApplyError::Protocol)?;
            let accepted =
                transport
                    .send_feature_report(&bytes)
                    .map_err(|error| ApplyError::Transport {
                        index,
                        detail: error.to_string(),
                    })?;
            if accepted != bytes.len() {
                return Err(ApplyError::InvalidTransportLength {
                    index,
                    actual: accepted,
                    expected: bytes.len(),
                });
            }
            sent += 1;
        }

        Ok(completed_apply_outcome(
            expected_report_lengths,
            sent,
            self.warnings.clone(),
        ))
    }

    /// Apply through the frame-oriented transport seam. Unsupported fields
    /// fail closed before the first transport call.
    pub fn apply<T>(&self, transport: &mut T) -> Result<ApplyOutcome, ApplyError>
    where
        T: ApplyTransport,
    {
        if !self.warnings.is_empty() {
            return Err(ApplyError::UnsupportedFields {
                warnings: self.warnings.clone(),
            });
        }
        if !self.verified_frames.is_empty() {
            return Err(ApplyError::VerifiedWritesRequireGatedApply);
        }
        if self.verified_sequence.is_some() {
            return Err(ApplyError::VerifiedSequenceRequiresGatedApply);
        }

        let expected_report_lengths = self
            .frames
            .iter()
            .map(ReportFrame::expected_len)
            .collect::<Vec<_>>();
        for (index, frame) in self.frames.iter().enumerate() {
            validate_sendable_frame(index, frame)?;
            transport
                .send(frame)
                .map_err(|error| ApplyError::Transport {
                    index,
                    detail: error.to_string(),
                })?;
        }

        Ok(completed_apply_outcome(
            expected_report_lengths,
            self.frames.len(),
            self.warnings.clone(),
        ))
    }

    /// Apply through a closure, useful for adapting an existing device seam
    /// whose error type cannot implement the local transport trait.
    pub fn apply_with<F, E>(&self, mut send: F) -> Result<ApplyOutcome, ApplyError>
    where
        F: FnMut(&ReportFrame) -> Result<(), E>,
        E: fmt::Display,
    {
        if !self.warnings.is_empty() {
            return Err(ApplyError::UnsupportedFields {
                warnings: self.warnings.clone(),
            });
        }
        if !self.verified_frames.is_empty() {
            return Err(ApplyError::VerifiedWritesRequireGatedApply);
        }
        if self.verified_sequence.is_some() {
            return Err(ApplyError::VerifiedSequenceRequiresGatedApply);
        }

        let expected_report_lengths = self
            .frames
            .iter()
            .map(ReportFrame::expected_len)
            .collect::<Vec<_>>();
        for (index, frame) in self.frames.iter().enumerate() {
            validate_sendable_frame(index, frame)?;
            frame.encode().map_err(ApplyError::Protocol)?;
            send(frame).map_err(|error| ApplyError::Transport {
                index,
                detail: error.to_string(),
            })?;
        }

        Ok(completed_apply_outcome(
            expected_report_lengths,
            self.frames.len(),
            self.warnings.clone(),
        ))
    }

    /// Apply evidence-backed tokens through a byte-oriented transport.
    ///
    /// Raw `frames` are rejected before any token is sent.  This keeps a
    /// manually injected observed frame from being mixed into an otherwise
    /// verified sequence.
    pub fn apply_verified_to<T>(&self, transport: &mut T) -> Result<ApplyOutcome, ApplyError>
    where
        T: VerifiedFeatureReportTransport,
    {
        self.validate_verified_plan()?;

        let mut sent = 0;
        let mut expected_report_lengths = Vec::with_capacity(self.verified_frames.len());
        for (index, write) in self.verified_frames.iter().enumerate() {
            let expected = write.as_bytes().map_err(ApplyError::Protocol)?.len();
            expected_report_lengths.push(expected);
            let actual =
                transport
                    .send_verified_write(write)
                    .map_err(|error| ApplyError::Transport {
                        index,
                        detail: error.to_string(),
                    })?;
            if actual != expected {
                return Err(ApplyError::InvalidTransportLength {
                    index,
                    actual,
                    expected,
                });
            }
            sent += 1;
        }

        Ok(completed_apply_outcome(
            expected_report_lengths,
            sent,
            self.warnings.clone(),
        ))
    }

    /// Apply evidence-backed tokens through a frame-oriented transport.
    pub fn apply_verified<T>(&self, transport: &mut T) -> Result<ApplyOutcome, ApplyError>
    where
        T: VerifiedApplyTransport,
    {
        self.validate_verified_plan()?;

        let expected_report_lengths = self
            .verified_frames
            .iter()
            .map(|write| write.as_bytes().map(|bytes| bytes.len()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ApplyError::Protocol)?;
        for (index, write) in self.verified_frames.iter().enumerate() {
            transport
                .send_verified_write(write)
                .map_err(|error| ApplyError::Transport {
                    index,
                    detail: error.to_string(),
                })?;
        }

        Ok(completed_apply_outcome(
            expected_report_lengths,
            self.verified_frames.len(),
            self.warnings.clone(),
        ))
    }

    /// Apply evidence-backed tokens through a closure, preserving the token
    /// type at the final caller-owned seam.
    pub fn apply_verified_with<F, E>(&self, mut send: F) -> Result<ApplyOutcome, ApplyError>
    where
        F: FnMut(&VerifiedWrite) -> Result<(), E>,
        E: fmt::Display,
    {
        self.validate_verified_plan()?;

        let expected_report_lengths = self
            .verified_frames
            .iter()
            .map(|write| write.as_bytes().map(|bytes| bytes.len()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(ApplyError::Protocol)?;
        for (index, write) in self.verified_frames.iter().enumerate() {
            send(write).map_err(|error| ApplyError::Transport {
                index,
                detail: error.to_string(),
            })?;
        }

        Ok(completed_apply_outcome(
            expected_report_lengths,
            self.verified_frames.len(),
            self.warnings.clone(),
        ))
    }

    /// Apply the complete evidence-backed sequence through a sequence-level
    /// transport gate. Raw frames are never handed to this transport trait.
    pub fn apply_verified_sequence<T>(&self, transport: &mut T) -> Result<ApplyOutcome, ApplyError>
    where
        T: VerifiedSequenceTransport,
    {
        let sequence = self.validated_sequence()?;
        let expected = sequence.len();
        let expected_report_lengths = sequence
            .frames()
            .iter()
            .map(ReportFrame::expected_len)
            .collect::<Vec<_>>();
        let actual = transport
            .send_verified_sequence(sequence)
            .map_err(|error| ApplyError::Transport {
                index: 0,
                detail: error.to_string(),
            })?;
        if actual != expected {
            return Err(ApplyError::InvalidSequenceCompletion { actual, expected });
        }

        Ok(completed_apply_outcome(
            expected_report_lengths,
            actual,
            self.warnings.clone(),
        ))
    }

    /// Apply the complete authorized sequence, then observe the exact
    /// PollingRate response through a read-only transport seam.
    ///
    /// The returned [`ApplyOutcome`] intentionally retains
    /// [`ApplyPersistence::NotObserved`].  A direct current-binary response
    /// proves an observation after this transfer, but it does not by itself
    /// prove reconnect/restart persistence.  Callers that need that stronger
    /// claim must retain an external, current-binary readback artifact.
    pub fn apply_verified_sequence_with_readback<T>(
        &self,
        transport: &mut T,
    ) -> Result<(ApplyOutcome, PollingRateReadback), ApplyError>
    where
        T: VerifiedSequenceTransport + PollingRateReadbackTransport,
    {
        let outcome = self.apply_verified_sequence(transport)?;
        let readback = self.observe_polling_rate_readback(transport)?;
        Ok((outcome, readback))
    }

    /// Observe the authorized sequence's PollingRate value without sending a
    /// standalone command frame.  This method is intended to be called after
    /// [`Self::apply_verified_sequence`]; it cannot be used to create or send
    /// an Apply token.
    pub fn observe_polling_rate_readback<T>(
        &self,
        transport: &mut T,
    ) -> Result<PollingRateReadback, ApplyError>
    where
        T: PollingRateReadbackTransport,
    {
        let sequence = self.validated_sequence()?;
        let readback = transport.observe_polling_rate_readback().map_err(|error| {
            ApplyError::ReadbackTransport {
                detail: error.to_string(),
            }
        })?;
        let expected = sequence.polling_rate_wire_value();
        if !readback.matches_wire_value(expected) {
            return Err(ApplyError::ReadbackMismatch {
                actual: readback.wire_value(),
                expected,
            });
        }
        Ok(readback)
    }

    /// Apply the complete sequence through a caller-owned, sequence-level
    /// adapter. The callback receives only the protocol authorization token,
    /// never a raw report.
    pub fn apply_verified_sequence_with<F, E>(
        &self,
        mut send: F,
    ) -> Result<ApplyOutcome, ApplyError>
    where
        F: FnMut(&VerifiedApplySequence) -> Result<usize, E>,
        E: fmt::Display,
    {
        let sequence = self.validated_sequence()?;
        let expected = sequence.len();
        let expected_report_lengths = sequence
            .frames()
            .iter()
            .map(ReportFrame::expected_len)
            .collect::<Vec<_>>();
        let actual = send(sequence).map_err(|error| ApplyError::Transport {
            index: 0,
            detail: error.to_string(),
        })?;
        if actual != expected {
            return Err(ApplyError::InvalidSequenceCompletion { actual, expected });
        }

        Ok(completed_apply_outcome(
            expected_report_lengths,
            actual,
            self.warnings.clone(),
        ))
    }

    fn validate_verified_plan(&self) -> Result<(), ApplyError> {
        if !self.warnings.is_empty() {
            return Err(ApplyError::UnsupportedFields {
                warnings: self.warnings.clone(),
            });
        }
        if self.verified_sequence.is_some() {
            return Err(ApplyError::VerifiedSequenceRequiresGatedApply);
        }
        if let Some((index, frame)) = self.frames.iter().enumerate().next() {
            return Err(ApplyError::UnsupportedFrame {
                index,
                report_id: frame.report_id,
                cmd: frame.cmd,
                subcmd: frame.subcmd,
            });
        }
        if let Some((index, write)) = self
            .verified_frames
            .iter()
            .enumerate()
            .find(|(_, write)| !write.is_registry_authorized())
        {
            let frame = write.frame();
            return Err(ApplyError::UnverifiedWrite {
                index,
                report_id: frame.report_id,
                cmd: frame.cmd,
                subcmd: frame.subcmd,
            });
        }
        Ok(())
    }

    fn validated_sequence(&self) -> Result<&VerifiedApplySequence, ApplyError> {
        if !self.warnings.is_empty() {
            return Err(ApplyError::UnsupportedFields {
                warnings: self.warnings.clone(),
            });
        }
        if let Some((index, frame)) = self.frames.iter().enumerate().next() {
            return Err(ApplyError::UnsupportedFrame {
                index,
                report_id: frame.report_id,
                cmd: frame.cmd,
                subcmd: frame.subcmd,
            });
        }
        if !self.verified_frames.is_empty() {
            return Err(ApplyError::VerifiedWritesRequireGatedApply);
        }
        let Some(sequence) = self.verified_sequence.as_ref() else {
            return Err(ApplyError::MissingVerifiedSequence);
        };
        VerifiedApplySequence::try_from_frames(sequence.frames())
            .map_err(ApplyError::InvalidVerifiedSequence)?;
        Ok(sequence)
    }
}

fn completed_apply_outcome(
    expected_report_lengths: Vec<usize>,
    completed_reports: usize,
    warnings: Vec<ApplyWarning>,
) -> ApplyOutcome {
    ApplyOutcome {
        sent: completed_reports,
        warnings,
        transfer: ApplyTransferRecord {
            expected_reports: expected_report_lengths.len(),
            completed_reports,
            expected_report_lengths,
        },
        persistence: ApplyPersistence::NotObserved,
    }
}

fn validate_sendable_frame(index: usize, frame: &ReportFrame) -> Result<(), ApplyError> {
    // A `ReportFrame` is a decode/dry-run value, never a write capability.
    // Even if a future registry entry marks its header writable, the frame
    // must be wrapped in a `VerifiedWrite` and sent through `apply_verified_*`.
    Err(ApplyError::UnsupportedFrame {
        index,
        report_id: frame.report_id,
        cmd: frame.cmd,
        subcmd: frame.subcmd,
    })
}

fn profile_has_key(profile: &Profile, key: &str) -> bool {
    let slot = profile.group_section();
    profile.doc.get(&slot, key).is_some() || profile.doc.get("GROUP", key).is_some()
}

fn profile_i32(profile: &Profile, key: &str) -> Option<i32> {
    let slot = profile.group_section();
    profile
        .doc
        .section(&slot)
        .and_then(|section| section.get_parse::<i32>(key))
        .or_else(|| {
            profile
                .doc
                .section("GROUP")
                .and_then(|section| section.get_parse::<i32>(key))
        })
}

fn authorized_polling_sequence(
    profile: &Profile,
) -> Result<Option<VerifiedApplySequence>, ApplyError> {
    if !profile_has_key(profile, "PollingRate") {
        return Ok(None);
    }

    let Some(profile_value) = profile_i32(profile, "PollingRate") else {
        return Ok(None);
    };
    let sequence = if profile_has_key(profile, "DPI") {
        let Some(dpi) = profile_i32(profile, "DPI") else {
            return Ok(None);
        };
        match dpi {
            // Zero is the captured baseline in the complete 156-report
            // sequence.  Keep the selected-DPI field at that baseline while
            // still allowing the independently authorized PollingRate value.
            0 => VerifiedApplySequence::for_profile_polling_rate(profile_value),
            dpi => VerifiedApplySequence::for_profile_polling_rate_and_dpi(profile_value, dpi),
        }
    } else {
        VerifiedApplySequence::for_profile_polling_rate(profile_value)
    };
    Ok(sequence.ok())
}

/// Build the sequence that the Rust-owned reconnect worker may replay.
///
/// The reconnect path deliberately ignores every profile field whose device
/// mapping is still unverified.  It accepts only the complete 125 Hz sequence
/// and, when present, the two A/B-proven selected-DPI values.  The captured
/// baseline `DPI=0` is also accepted, but it never grants a selected-DPI
/// mapping: the sequence leaves that byte at its observed baseline.  Any
/// other unsupported value disables replay instead of silently changing it to
/// a different DPI.
pub fn build_rust_owned_reconnect_sequence(profile: &Profile) -> Option<VerifiedApplySequence> {
    if !profile_has_key(profile, "PollingRate") {
        return None;
    }

    let polling_rate = profile_i32(profile, "PollingRate")?;
    if profile_has_key(profile, "DPI") {
        let dpi = profile_i32(profile, "DPI")?;
        match dpi {
            0 => VerifiedApplySequence::for_profile_polling_rate(polling_rate).ok(),
            1 | 2 => {
                VerifiedApplySequence::for_profile_polling_rate_and_dpi(polling_rate, dpi).ok()
            }
            _ => None,
        }
    } else {
        VerifiedApplySequence::for_profile_polling_rate(polling_rate).ok()
    }
}

fn collect_profile_warnings(profile: &Profile, warnings: &mut Vec<ApplyWarning>) {
    collect_profile_warnings_with_polling(profile, warnings, true);
}

fn collect_profile_warnings_with_polling(
    profile: &Profile,
    warnings: &mut Vec<ApplyWarning>,
    include_polling_rate: bool,
) {
    // Keep this list explicit and ordered.  It is a protocol-safety list,
    // rather than a generic scan of INI keys, so unknown metadata remains
    // lossless and cannot accidentally become a device write.
    const SETTINGS: &[(&str, &str)] = &[
        (
            "GroupID",
            "profile metadata has no verified device write mapping",
        ),
        (
            "Group",
            "profile metadata has no verified device write mapping",
        ),
        (
            "MouseSensitivity",
            "profile setting has no verified frame mapping",
        ),
        (
            "WheelScrollLines",
            "profile setting has no verified frame mapping",
        ),
        (
            "UniversalScrollEnabled",
            "profile setting has no verified frame mapping",
        ),
        (
            "OFASensitivity",
            "profile setting has no verified frame mapping",
        ),
        (
            "DoubleClickSpeed",
            "profile setting has no verified frame mapping",
        ),
        (
            "Acceleration",
            "profile setting has no verified frame mapping",
        ),
        (
            "XYSensitivityEnabled",
            "profile setting has no verified frame mapping",
        ),
        (
            "MouseSpeedX",
            "profile setting has no verified frame mapping",
        ),
        (
            "MouseSpeedY",
            "profile setting has no verified frame mapping",
        ),
        ("LedState1", "profile setting has no verified frame mapping"),
        (
            "FlashState1",
            "profile setting has no verified frame mapping",
        ),
        (
            "LedColor1",
            "COLORREF conversion is known, but device write mapping is unverified",
        ),
        ("ANGLE", "profile setting has no verified frame mapping"),
        ("LIFT", "profile setting has no verified frame mapping"),
        ("DPI", "profile setting has no verified frame mapping"),
        (
            "PollingRate",
            "polling-rate numeric mapping is unconfirmed; no write emitted",
        ),
        (
            "DPIStageNum",
            "DPI display/stage mapping is unconfirmed; no write emitted",
        ),
        (
            "DPIStageValue",
            "DPI display/code mapping is unconfirmed; no write emitted",
        ),
        (
            "DPICurrentX",
            "DPI current-stage mapping is unconfirmed; no write emitted",
        ),
        (
            "DPICurrentY",
            "DPI current-stage mapping is unconfirmed; no write emitted",
        ),
        (
            "LedMode1",
            "LED mode numeric mapping is unconfirmed; no write emitted",
        ),
        (
            "BreathState1",
            "LED breath numeric mapping is unconfirmed; no write emitted",
        ),
    ];

    for (field, detail) in SETTINGS {
        if *field == "PollingRate" && !include_polling_rate {
            continue;
        }
        if profile_has_key(profile, field) {
            warnings.push(ApplyWarning::new(*field, *detail));
        }
    }

    // Button functions are known in the profile model, but their device
    // command encoding is not.  IDs 48/49/52/53 receive an explicit reason
    // because they are called out as unconfirmed subfunctions in the phase
    // requirements.
    for button_id in 1..=20 {
        let assignment = profile.button(button_id);
        if assignment.func == 255 {
            continue;
        }
        let detail = match assignment.func {
            48 | 49 | 52 | 53 => format!(
                "button {button_id} uses unverified subfunction id {}; no write emitted",
                assignment.func
            ),
            function => format!(
                "button {button_id} function id {function} has no verified frame mapping; no write emitted"
            ),
        };
        warnings.push(ApplyWarning::new("ButtonFunc", detail));
    }
}

/// Convenience free function for callers that prefer functional APIs.
pub fn dry_run(profile: &Profile) -> ApplyPlan {
    ApplyPlan::dry_run(profile)
}

/// Fallible convenience free function with default options.
pub fn build_apply_plan(profile: &Profile) -> Result<ApplyPlan, ApplyError> {
    ApplyPlan::try_build(profile)
}

/// Build the explicit, protocol-authorized profile plan.
pub fn build_authorized_apply_plan(profile: &Profile) -> Result<ApplyPlan, ApplyError> {
    ApplyPlan::try_build_authorized(profile)
}

/// Fallible convenience free function with explicit options.
pub fn build_apply_plan_with_options(
    profile: &Profile,
    options: ApplyOptions,
) -> Result<ApplyPlan, ApplyError> {
    ApplyPlan::try_build_with_options(profile, options)
}

/// Explicit, transport-backed application entry point.
pub fn apply_profile<T>(profile: &Profile, transport: &mut T) -> Result<ApplyOutcome, ApplyError>
where
    T: ApplyTransport,
{
    ApplyPlan::try_build(profile)?.apply(transport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_protocol::{CMD_F2, REPORT_ID_CONFIG, SUBCMD_F2_STATUS};
    use crate::ini::IniDoc;
    use crate::profile::Profile;

    #[test]
    fn empty_profile_records_status_query_without_adding_a_write() {
        let profile = Profile {
            slot: 1,
            doc: IniDoc::default(),
        };

        let plan = ApplyPlan::dry_run(&profile);

        assert_eq!(plan.frames.len(), 0);
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.phases(), &[]);
        let status = plan.status_query().expect("observed status query");
        assert_eq!(
            (status.report_id, status.cmd, status.subcmd),
            (REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_STATUS)
        );
    }

    #[test]
    fn default_profile_reports_unverified_fields_without_speculative_writes() {
        let plan = ApplyPlan::dry_run(&Profile::default_profile(1));

        assert_eq!(plan.frames.len(), 0);
        assert!(plan.status_query().is_some());
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.field == "PollingRate"));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.field == "DPIStageValue"));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.field == "LedMode1"));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.field == "BreathState1"));
        assert!(plan.frames.is_empty());
    }

    #[test]
    fn unsupported_subfunctions_are_warnings_in_button_order() {
        let mut profile = Profile::default_profile(1);
        profile.set_button_func(1, 48);
        profile.set_button_func(2, 49);
        profile.set_button_func(3, 52);
        profile.set_button_func(4, 53);

        let plan = ApplyPlan::dry_run(&profile);
        let button_warnings: Vec<_> = plan
            .warnings
            .iter()
            .filter(|warning| warning.field == "ButtonFunc")
            .collect();

        assert!(button_warnings
            .iter()
            .any(|warning| warning.detail.contains("button 1")));
        assert!(button_warnings
            .iter()
            .any(|warning| warning.detail.contains("button 2")));
        assert!(button_warnings
            .iter()
            .any(|warning| warning.detail.contains("button 3")));
        assert!(button_warnings
            .iter()
            .any(|warning| warning.detail.contains("button 4")));
        assert!(plan.frames.is_empty());
    }

    #[test]
    fn dry_run_is_deterministic() {
        let profile = Profile::default_profile(2);

        assert_eq!(ApplyPlan::dry_run(&profile), ApplyPlan::dry_run(&profile));
    }

    #[test]
    fn apply_is_explicit_and_uses_the_transport_seam() {
        let profile = Profile {
            slot: 1,
            doc: IniDoc::default(),
        };
        let plan = ApplyPlan::dry_run(&profile);
        let mut sent = Vec::new();

        let result = plan.apply_with(|frame| {
            sent.push(frame.encode().expect("the plan contains valid frames"));
            Ok::<(), &'static str>(())
        });

        assert_eq!(result.expect("empty profile has no writes").sent, 0);
        assert!(sent.is_empty());
        assert!(plan.status_query().is_some());
    }

    #[test]
    fn device_transport_adapter_rejects_observed_extended_length_without_a_gate() {
        let mut transport = crate::device::DeviceTransport::mock();
        let mut bytes = vec![0; crate::device::EXTENDED_CONTROL_REPORT_LENGTH];
        bytes[..3].copy_from_slice(&[0x03, 0xF3, 0x20]);

        let error =
            <crate::device::DeviceTransport as FeatureReportTransport>::send_feature_report(
                &mut transport,
                &bytes,
            )
            .expect_err("the adapter must not bypass the reviewed write gate");
        assert_eq!(
            error,
            crate::device::DeviceError::UnverifiedReportHeader {
                report_id: 0x03,
                cmd: 0xF3,
                subcmd: 0x20,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn manually_injected_status_query_is_rejected_by_write_apply() {
        let plan = ApplyPlan {
            frames: vec![status_query_frame().expect("status query")],
            ..ApplyPlan::default()
        };
        let mut sent = Vec::new();
        let error = plan
            .apply_with(|frame| {
                sent.push(frame.clone());
                Ok::<(), &'static str>(())
            })
            .expect_err("status traffic must not use the write transport");
        assert!(matches!(
            error,
            ApplyError::UnsupportedFrame { index: 0, .. }
        ));
        assert!(sent.is_empty());
    }

    #[test]
    fn manually_injected_observed_but_unverified_frame_is_rejected_by_write_apply() {
        let frame = ReportFrame::new(REPORT_ID_CONFIG, 0xF1, 0x01, [0; PARAM_LEN])
            .expect("observed frame shape");
        let plan = ApplyPlan {
            frames: vec![frame],
            ..ApplyPlan::default()
        };
        let mut sent = Vec::new();

        let error = plan
            .apply_with(|frame| {
                sent.push(frame.clone());
                Ok::<(), &'static str>(())
            })
            .expect_err("observed command is not a verified write");
        assert!(matches!(
            error,
            ApplyError::UnsupportedFrame {
                index: 0,
                report_id: REPORT_ID_CONFIG,
                cmd: 0xF1,
                subcmd: 0x01,
            }
        ));
        assert!(sent.is_empty());
    }

    #[test]
    fn verified_tokens_require_the_gated_apply_path() {
        let mapping = crate::device_protocol::test_verified_command_mapping(
            REPORT_ID_CONFIG,
            0xF1,
            0x01,
            crate::device_protocol::REPORT_LEN,
        );
        let write = mapping
            .frame([0xA5; PARAM_LEN])
            .expect("test mapping produces a validated token");
        let plan = ApplyPlan::default().with_verified_writes([write]);
        let mut called = false;

        let error = plan
            .apply_with(|_| {
                called = true;
                Ok::<(), &'static str>(())
            })
            .expect_err("legacy raw apply must not consume a verified token");

        assert_eq!(error, ApplyError::VerifiedWritesRequireGatedApply);
        assert!(!called);
    }

    #[test]
    fn unregistered_verified_tokens_do_not_reach_the_gated_closure() {
        let mapping = crate::device_protocol::test_verified_command_mapping(
            REPORT_ID_CONFIG,
            0xF1,
            0x01,
            crate::device_protocol::REPORT_LEN,
        );
        let write = mapping
            .frame([0x5A; PARAM_LEN])
            .expect("test mapping produces a validated token");
        let plan = ApplyPlan::default().with_verified_writes([write]);
        let mut sent = Vec::new();

        let error = plan
            .apply_verified_with(|write| {
                sent.push(write.as_bytes().expect("token is already validated"));
                Ok::<(), &'static str>(())
            })
            .expect_err("unregistered tokens must fail before the closure");

        assert!(matches!(
            error,
            ApplyError::UnverifiedWrite { index: 0, .. }
        ));
        assert!(sent.is_empty());
    }

    #[test]
    fn device_transport_rejects_an_unregistered_verified_token() {
        let mapping = crate::device_protocol::test_verified_command_mapping(
            REPORT_ID_CONFIG,
            0xF1,
            0x01,
            crate::device_protocol::REPORT_LEN,
        );
        let write = mapping
            .frame([0x3C; PARAM_LEN])
            .expect("test mapping produces a validated token");
        let plan = ApplyPlan::default().with_verified_writes([write]);
        let mut transport = crate::device::DeviceTransport::mock();

        let error = plan
            .apply_verified_to(&mut transport)
            .expect_err("the transport boundary must enforce registry membership");

        assert!(matches!(
            error,
            ApplyError::UnverifiedWrite { index: 0, .. }
        ));
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn verified_apply_rejects_mixed_raw_frames_before_token_io() {
        let mapping = crate::device_protocol::test_verified_command_mapping(
            REPORT_ID_CONFIG,
            0xF1,
            0x01,
            crate::device_protocol::REPORT_LEN,
        );
        let write = mapping
            .frame([0x5A; PARAM_LEN])
            .expect("test mapping produces a validated token");
        let raw = ReportFrame::new(REPORT_ID_CONFIG, 0xF1, 0x01, [0; PARAM_LEN])
            .expect("raw frame shape");
        let plan = ApplyPlan {
            frames: vec![raw],
            ..ApplyPlan::default().with_verified_writes([write])
        };
        let mut called = false;

        let error = plan
            .apply_verified_with(|_| {
                called = true;
                Ok::<(), &'static str>(())
            })
            .expect_err("raw and token frames must not be mixed");

        assert!(matches!(
            error,
            ApplyError::UnsupportedFrame { index: 0, .. }
        ));
        assert!(!called);
    }

    #[test]
    fn try_attaching_verified_writes_rejects_raw_frames_before_building_a_plan() {
        let mapping = crate::device_protocol::test_verified_command_mapping(
            REPORT_ID_CONFIG,
            0xF1,
            0x01,
            crate::device_protocol::REPORT_LEN,
        );
        let write = mapping
            .frame([0x5A; PARAM_LEN])
            .expect("test mapping produces a validated token");
        let raw = ReportFrame::new(REPORT_ID_CONFIG, 0xF1, 0x01, [0; PARAM_LEN])
            .expect("raw frame shape");
        let plan = ApplyPlan {
            frames: vec![raw],
            ..ApplyPlan::default()
        };

        let error = plan
            .try_with_verified_writes([write])
            .expect_err("a verified plan must never contain raw frames");

        assert!(matches!(
            error,
            ApplyError::UnsupportedFrame {
                index: 0,
                report_id: REPORT_ID_CONFIG,
                cmd: 0xF1,
                subcmd: 0x01,
            }
        ));
    }

    #[test]
    fn write_ready_requires_an_explicit_verified_token_sequence() {
        let empty_profile = Profile {
            slot: 1,
            doc: IniDoc::default(),
        };
        let dry_run = ApplyPlan::dry_run(&empty_profile);
        assert!(!dry_run.is_write_ready());

        let mapping = crate::device_protocol::test_verified_command_mapping(
            REPORT_ID_CONFIG,
            0xF1,
            0x01,
            crate::device_protocol::REPORT_LEN,
        );
        let write = mapping
            .frame([0xA5; PARAM_LEN])
            .expect("test mapping produces a validated token");
        let ready = ApplyPlan::default().with_verified_writes([write]);
        assert!(!ready.is_write_ready());

        let warned = ApplyPlan {
            warnings: vec![ApplyWarning::new("field", "unverified")],
            ..ready
        };
        assert!(!warned.is_write_ready());
    }

    #[test]
    fn authorized_constructor_builds_the_reviewed_125hz_sequence() {
        let mut doc = IniDoc::default();
        doc.set("GROUP0", "PollingRate", "8");
        let profile = Profile { slot: 1, doc };

        let plan = ApplyPlan::try_build_authorized(&profile)
            .expect("authorized profile planning should be fallible only for protocol failures");
        let sequence = plan
            .verified_sequence()
            .expect("the audited 125 Hz profile value must attach a sequence");

        assert_eq!(
            sequence.len(),
            crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT
        );
        assert!(plan.frames.is_empty());
        assert!(plan.verified_frames().is_empty());
        assert!(plan.warnings.is_empty());
        assert!(plan.is_write_ready());
        assert_eq!(
            sequence.frames()[crate::device_protocol::APPLY_POLLING_RATE_FRAME_INDEX].params
                [crate::device_protocol::POLLING_RATE_OBSERVED_OFFSET
                    - crate::device_protocol::PARAMS_OFFSET],
            crate::device_protocol::APPLY_POLLING_RATE_WIRE_VALUE_125_HZ
        );
    }

    #[test]
    fn authorized_constructor_rejects_unmapped_scalar_fields() {
        let mut doc = IniDoc::default();
        doc.set("GROUP0", "PollingRate", "8");
        doc.set("GROUP0", "WheelScrollLines", "1");
        doc.set("GROUP0", "UniversalScrollEnabled", "0");
        doc.set("GROUP0", "FlashState1", "5");
        let profile = Profile { slot: 1, doc };

        let plan = ApplyPlan::try_build_authorized(&profile)
            .expect("unmapped scalar fields remain representable as warnings");

        for field in ["WheelScrollLines", "UniversalScrollEnabled", "FlashState1"] {
            assert!(
                plan.warnings
                    .iter()
                    .any(|warning| warning.field_name() == field),
                "{field} must block the complete sequence"
            );
        }
        assert!(!plan.is_write_ready());
    }

    #[test]
    fn authorized_constructor_keeps_unproven_polling_values_as_warnings() {
        let mut doc = IniDoc::default();
        doc.set("GROUP0", "PollingRate", "4");
        let profile = Profile { slot: 1, doc };

        let plan = ApplyPlan::try_build_authorized(&profile)
            .expect("unsupported profile values remain representable as warnings");

        assert!(plan.verified_sequence().is_none());
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.field_name() == "PollingRate"));
        assert!(!plan.is_write_ready());
    }

    #[test]
    fn dry_run_keeps_the_existing_polling_warning_and_never_attaches_a_sequence() {
        let mut doc = IniDoc::default();
        doc.set("GROUP0", "PollingRate", "8");
        let profile = Profile { slot: 1, doc };

        let plan = ApplyPlan::dry_run(&profile);

        assert!(plan.verified_sequence().is_none());
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.field_name() == "PollingRate"));
        assert!(plan.frames.is_empty());
    }

    #[derive(Default)]
    struct CountingSequenceTransport {
        calls: usize,
        sequence_lengths: Vec<usize>,
        acknowledged: usize,
    }

    impl VerifiedSequenceTransport for CountingSequenceTransport {
        type Error = &'static str;

        fn send_verified_sequence(
            &mut self,
            sequence: &VerifiedApplySequence,
        ) -> Result<usize, Self::Error> {
            self.calls += 1;
            self.sequence_lengths.push(sequence.len());
            Ok(self.acknowledged)
        }
    }

    #[test]
    fn sequence_apply_passes_only_the_authorization_token_and_requires_all_completions() {
        let mut doc = IniDoc::default();
        doc.set("GROUP0", "PollingRate", "8");
        let profile = Profile { slot: 1, doc };
        let plan = ApplyPlan::try_build_authorized(&profile).expect("authorized profile");

        let mut transport = CountingSequenceTransport {
            acknowledged: crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT,
            ..Default::default()
        };
        let outcome = plan
            .apply_verified_sequence(&mut transport)
            .expect("the complete sequence should be accepted");

        assert_eq!(
            outcome.sent,
            crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT
        );
        assert_eq!(transport.calls, 1);
        assert_eq!(
            transport.sequence_lengths,
            vec![crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT]
        );

        let mut incomplete = CountingSequenceTransport {
            acknowledged: crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT - 1,
            ..Default::default()
        };
        assert!(matches!(
            plan.apply_verified_sequence(&mut incomplete),
            Err(ApplyError::InvalidSequenceCompletion { .. })
        ));
        assert_eq!(incomplete.calls, 1);
    }

    #[test]
    fn authorized_sequence_result_records_transfer_shape_without_persistence_claim() {
        let mut doc = IniDoc::default();
        doc.set("GROUP0", "PollingRate", "8");
        let profile = Profile { slot: 1, doc };
        let plan = ApplyPlan::try_build_authorized(&profile).expect("authorized profile");

        let mut transport = CountingSequenceTransport {
            acknowledged: crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT,
            ..Default::default()
        };
        let outcome = plan
            .apply_verified_sequence(&mut transport)
            .expect("the complete sequence should be accepted");

        assert_eq!(
            outcome.transfer.expected_reports,
            crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT
        );
        assert_eq!(
            outcome.transfer.completed_reports,
            crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT
        );
        assert_eq!(
            outcome
                .transfer
                .expected_report_lengths
                .iter()
                .filter(|length| **length == crate::device_protocol::REPORT_LEN)
                .count(),
            155
        );
        assert_eq!(
            outcome
                .transfer
                .expected_report_lengths
                .iter()
                .filter(|length| **length == crate::device_protocol::EXTENDED_REPORT_LEN)
                .count(),
            1
        );
        assert!(outcome.transfer.is_complete());
        assert_eq!(outcome.persistence, ApplyPersistence::NotObserved);
        assert!(!outcome.persistence.is_verified());
    }

    #[test]
    fn legacy_raw_apply_entrypoints_reject_an_authorized_sequence_before_io() {
        let mut doc = IniDoc::default();
        doc.set("GROUP0", "PollingRate", "8");
        let profile = Profile { slot: 1, doc };
        let plan = ApplyPlan::try_build_authorized(&profile).expect("authorized profile");
        let mut transport = CountingSequenceTransport {
            acknowledged: crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT,
            ..Default::default()
        };

        let error = plan
            .apply_with(|_| {
                transport.calls += 1;
                Ok::<(), &'static str>(())
            })
            .expect_err("an ordered token must not use a raw-frame closure");

        assert_eq!(error, ApplyError::VerifiedSequenceRequiresGatedApply);
        assert_eq!(transport.calls, 0);

        let mut device = crate::device::DeviceTransport::mock();
        let error = plan
            .apply_verified_to(&mut device)
            .expect_err("a complete sequence must not use an individual-token seam");
        assert_eq!(error, ApplyError::VerifiedSequenceRequiresGatedApply);
        assert!(device.sent_mock_reports().unwrap().is_empty());
    }

    #[derive(Default)]
    struct CountingApplyTransport {
        calls: usize,
    }

    impl ApplyTransport for CountingApplyTransport {
        type Error = &'static str;

        fn send(&mut self, _frame: &ReportFrame) -> Result<(), Self::Error> {
            self.calls += 1;
            Ok(())
        }
    }

    impl FeatureReportTransport for CountingApplyTransport {
        type Error = &'static str;

        fn send_feature_report(&mut self, _bytes: &[u8]) -> Result<usize, Self::Error> {
            self.calls += 1;
            Ok(0)
        }
    }

    #[test]
    fn every_raw_apply_entrypoint_rejects_before_transport_io() {
        let raw = ReportFrame::new(REPORT_ID_CONFIG, 0xF1, 0x01, [0; PARAM_LEN])
            .expect("raw frame shape");
        let plan = ApplyPlan {
            frames: vec![raw],
            ..ApplyPlan::default()
        };

        let mut frame_transport = CountingApplyTransport::default();
        assert!(matches!(
            plan.apply(&mut frame_transport),
            Err(ApplyError::UnsupportedFrame { index: 0, .. })
        ));
        assert_eq!(frame_transport.calls, 0);

        let mut byte_transport = CountingApplyTransport::default();
        assert!(matches!(
            plan.apply_to(&mut byte_transport),
            Err(ApplyError::UnsupportedFrame { index: 0, .. })
        ));
        assert_eq!(byte_transport.calls, 0);
    }
}
