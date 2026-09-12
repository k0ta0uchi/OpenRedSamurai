//! Wire-level schema for the RED SAMURAI vendor HID feature reports.
//!
//! The reverse-engineering notes describe the common feature report as
//! `[report_id, cmd, subcmd, params...]`, with a 16-byte report in normal
//! use.  The controlled USBPcap2 trace also contains one `03/F3/20` report
//! whose buffer is exactly 64 bytes; the disassembly-backed `04/F3/C8` block
//! remains a separate 0x400-byte observation.  This module deliberately stops at the
//! wire schema: it does not open HID devices.  The sequence token below is a
//! complete, evidence-backed Apply authorization; it is not a general
//! profile-to-command mapping.  Every standalone command whose parameter
//! meaning has not been verified is therefore exposed as `Unsupported`.

use core::fmt;

/// Number of bytes occupied by the report id, command, and subcommand.
pub const FRAME_HEADER_LEN: usize = 3;
/// Offset of the report id in an encoded buffer.
pub const REPORT_ID_OFFSET: usize = 0;
/// Offset of the command byte in an encoded buffer.
pub const CMD_OFFSET: usize = 1;
/// Offset of the subcommand byte in an encoded buffer.
pub const SUBCMD_OFFSET: usize = 2;
/// Offset at which command parameters begin.
pub const PARAMS_OFFSET: usize = FRAME_HEADER_LEN;

/// Length of the ordinary feature report observed in `ANALYSIS.md`.
pub const REPORT_LEN: usize = 16;
/// Alias for callers that use the HID terminology.
pub const FEATURE_REPORT_LEN: usize = REPORT_LEN;
/// Alias for the ordinary report size.
pub const STANDARD_REPORT_LEN: usize = REPORT_LEN;
/// Number of parameter bytes in an ordinary report.
pub const PARAM_LEN: usize = REPORT_LEN - FRAME_HEADER_LEN;
/// Alias for the ordinary parameter capacity.
pub const STANDARD_PARAM_LEN: usize = PARAM_LEN;

/// Length of the extended report observed in the controlled USBPcap2
/// captures (`03/F3/20`).  This is a captured wire shape, not a permission
/// to send the report: its field semantics and the complete apply sequence
/// remain unresolved.
pub const EXTENDED_REPORT_LEN: usize = 64;
/// Number of parameter bytes in the observed 64-byte report.
pub const EXTENDED_PARAM_LEN: usize = EXTENDED_REPORT_LEN - FRAME_HEADER_LEN;

/// Length of the observed `04/F3/C8` block report.
pub const BULK_REPORT_LEN: usize = 0x400;
/// Number of parameter bytes in the observed block report.
pub const BULK_PARAM_LEN: usize = BULK_REPORT_LEN - FRAME_HEADER_LEN;

/// Length of the exact PollingRate response observed on the official
/// post-Apply GET_REPORT route.  This is a response length, not a new write
/// shape; the corresponding request route remains the observed 16-byte
/// configuration route.
pub const POLLING_RATE_READBACK_LEN: usize = 13;
/// Full response offset of the observed PollingRate wire value.
pub const POLLING_RATE_READBACK_VALUE_OFFSET: usize = 8;

/// Length of the exact selected-DPI response observed on the official
/// post-Apply GET_REPORT route.  The request route is the 64-byte report-ID
/// route for report 03; the device returns this shorter 40-byte response.
pub const DPI_SELECTION_READBACK_LEN: usize = 40;
/// Full response offset of the selected-DPI wire value.
pub const DPI_SELECTION_READBACK_VALUE_OFFSET: usize = 8;
/// Report ID used by the selected-DPI GET_REPORT route.
pub const DPI_SELECTION_READBACK_REPORT_ID: u8 = REPORT_ID_CONFIG_EXTENDED;
/// Request buffer length used by the selected-DPI GET_REPORT route.
pub const DPI_SELECTION_READBACK_REQUEST_LEN: usize = EXTENDED_REPORT_LEN;

/// Report id used by the ordinary configuration/status traffic.
pub const REPORT_ID_CONFIG: u8 = 0x02;
/// Report id used by the second ordinary configuration interface.
pub const REPORT_ID_CONFIG_EXTENDED: u8 = 0x03;
/// Report id used by the observed 1024-byte block transfer.
pub const REPORT_ID_BULK: u8 = 0x04;
/// Report id observed with the unresolved `(04, 01, 00)` prefix.
pub const REPORT_ID_UNCLASSIFIED: u8 = 0x05;

// Numeric aliases are useful when comparing a trace with the source report.
pub const REPORT_ID_02: u8 = REPORT_ID_CONFIG;
pub const REPORT_ID_03: u8 = REPORT_ID_CONFIG_EXTENDED;
pub const REPORT_ID_04: u8 = REPORT_ID_BULK;
pub const REPORT_ID_05: u8 = REPORT_ID_UNCLASSIFIED;

/// Commands observed in the disassembly-backed report table.
pub const CMD_F1: u8 = 0xF1;
pub const CMD_F2: u8 = 0xF2;
pub const CMD_F3: u8 = 0xF3;
pub const CMD_F5: u8 = 0xF5;

// Descriptive aliases for callers that prefer `COMMAND_*` names.
pub const COMMAND_F1: u8 = CMD_F1;
pub const COMMAND_F2: u8 = CMD_F2;
pub const COMMAND_F3: u8 = CMD_F3;
pub const COMMAND_F5: u8 = CMD_F5;

/// F2 subcommands observed on report id 02/03.
pub const SUBCMD_F2_STATUS: u8 = 0x2C;
pub const SUBCMD_F2_04: u8 = 0x04;
pub const SUBCMD_F2_20: u8 = 0x20;
pub const SUBCMD_F2_32: u8 = 0x32;
pub const SUBCMD_F2_38: u8 = 0x38;
pub const SUBCMD_F2_3D: u8 = 0x3D;

/// F3 subcommands observed on report id 02/03, plus the bulk subcommand.
pub const SUBCMD_F3_20: u8 = 0x20;
pub const SUBCMD_F3_2C: u8 = 0x2C;
pub const SUBCMD_F3_32: u8 = 0x32;
pub const SUBCMD_F3_38: u8 = 0x38;
pub const SUBCMD_F3_42: u8 = 0x42;
pub const SUBCMD_F3_44: u8 = 0x44;
pub const SUBCMD_F3_46: u8 = 0x46;
pub const SUBCMD_F3_49: u8 = 0x49;
pub const SUBCMD_F3_4F: u8 = 0x4F;
pub const SUBCMD_F3_5C: u8 = 0x5C;
pub const SUBCMD_F3_8E: u8 = 0x8E;
pub const SUBCMD_F3_BULK: u8 = 0xC8;

// UI-correlated headers from the additional complete Apply captures.  These
// aliases describe where a delta was observed; they do not grant write
// permission or claim a universal field encoding.
pub const SUBCMD_F3_DPI_VALUE_OBSERVED: u8 = SUBCMD_F3_44;
pub const SUBCMD_F3_DPI_SELECTION_OBSERVED: u8 = SUBCMD_F3_42;
pub const SUBCMD_F3_LIGHT_MODE_COLOR_OBSERVED: u8 = SUBCMD_F3_49;
pub const SUBCMD_F3_LIGHT_BRIGHTNESS_OBSERVED: u8 = SUBCMD_F3_4F;
pub const SUBCMD_F3_DPI_ENABLE_OBSERVED: u8 = SUBCMD_F3_5C;
pub const SUBCMD_F3_BUTTON_FUNCTION_OBSERVED: u8 = SUBCMD_F3_8E;

/// F1/F5 subcommands observed in the commit/reset family.
pub const SUBCMD_F1_01: u8 = 0x01;
pub const SUBCMD_F1_02: u8 = 0x02;
pub const SUBCMD_F1_04: u8 = 0x04;
pub const SUBCMD_F1_08: u8 = 0x08;
pub const SUBCMD_F1_10: u8 = 0x10;
pub const SUBCMD_F5_00: u8 = 0x00;
pub const SUBCMD_F5_01: u8 = 0x01;
pub const SUBCMD_F5_04: u8 = 0x04;
pub const SUBCMD_F5_08: u8 = 0x08;

// Generic numeric aliases for the overlapping subcommand values.
pub const SUBCMD_00: u8 = 0x00;
pub const SUBCMD_01: u8 = 0x01;
pub const SUBCMD_04: u8 = 0x04;
pub const SUBCMD_08: u8 = 0x08;
pub const SUBCMD_10: u8 = 0x10;
pub const SUBCMD_20: u8 = 0x20;
pub const SUBCMD_2C: u8 = 0x2C;
pub const SUBCMD_32: u8 = 0x32;
pub const SUBCMD_38: u8 = 0x38;
pub const SUBCMD_3D: u8 = 0x3D;
pub const SUBCMD_44: u8 = 0x44;
pub const SUBCMD_46: u8 = 0x46;
pub const SUBCMD_49: u8 = 0x49;
pub const SUBCMD_4F: u8 = 0x4F;
pub const SUBCMD_5C: u8 = 0x5C;
pub const SUBCMD_8E: u8 = 0x8E;
pub const SUBCMD_C8: u8 = 0xC8;

/// The report header that changed in both controlled polling-rate captures.
/// The byte at full-report offset 8 changed from `04` to `01` (or back) while
/// the other 155 reports were byte-for-byte identical.  This constant names
/// the observation without assigning a universal numeric rate encoding.
pub const SUBCMD_F3_POLLING_RATE_OBSERVED: u8 = SUBCMD_F3_32;
/// Full-report offset of the correlated polling-rate byte in `02/F3/32`.
pub const POLLING_RATE_OBSERVED_OFFSET: usize = 8;

/// Number of reports in the complete physical 125 Hz Apply burst.
pub const APPLY_SEQUENCE_FRAME_COUNT: usize = 156;
/// Alias using the HID report terminology.
pub const APPLY_SEQUENCE_LEN: usize = APPLY_SEQUENCE_FRAME_COUNT;
/// Zero-based report index of the `02/F3/32` PollingRate substitution.
pub const APPLY_POLLING_RATE_FRAME_INDEX: usize = 13;
/// Alias for callers that refer to the substitution as a report index.
pub const APPLY_POLLING_RATE_INDEX: usize = APPLY_POLLING_RATE_FRAME_INDEX;
/// Profile value authorized by the physical 125 Hz Apply capture.
pub const APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ: i32 = 8;
/// Wire value authorized at [`APPLY_POLLING_RATE_FRAME_INDEX`].
pub const APPLY_POLLING_RATE_WIRE_VALUE_125_HZ: u8 = 0x08;
/// Zero-based report index of the selected-DPI substitution in the complete
/// Apply sequence.
pub const APPLY_DPI_SELECTION_FRAME_INDEX: usize = 16;
/// Full-report offset of the selected-DPI byte in `02/F3/42`.
pub const DPI_SELECTION_APPLY_OBSERVED_OFFSET: usize = 8;
/// Profile values covered by the controlled selected-DPI A/B capture.
pub const AUTHORIZED_DPI_SELECTION_PROFILE_VALUES: &[i32] = &[1, 2];
/// Zero-based report index of the observed `02/F3/49` LED mode/color report.
pub const APPLY_LIGHT_MODE_FRAME_INDEX: usize = 3;
/// Profile value used by the UI for the rainbow LED mode.
pub const APPLY_LIGHT_MODE_PROFILE_VALUE_RAINBOW: i32 = 2;
/// The two mode bytes observed when the official GUI applies レインボー.
/// The surrounding color and state bytes remain part of the complete
/// evidence-backed sequence.
pub const APPLY_LIGHT_MODE_RAINBOW_WIRE_PAIR: [u8; 2] = [0x03, 0x05];
/// Evidence reference for the selected-DPI sequence/readback promotion.
pub const DPI_SELECTION_EVIDENCE: &str =
    "captures/dpi-reconnect-readback-comparison-20260911.md: Run A/B Apply -> reconnect -> readback";
/// Evidence reference for the sequence token.
pub const APPLY_SEQUENCE_EVIDENCE: &str =
    "captures/hardware-evidence-20260909T071720685Z-5d9fdff2/evidence-report.md: Official 125Hz Apply";
/// Evidence reference for the official GUI rainbow Apply capture.
pub const LIGHT_MODE_RAINBOW_EVIDENCE: &str =
    "captures/official-light-rainbow-interactive-20260912-223739/root2.pcap: complete Apply with レインボー selected";

/// The unresolved report-05 observation was `(04, 01, 00)` after the
/// report id.  Its field meaning is intentionally not interpreted.
pub const REPORT_05_OBSERVED_PREFIX: [u8; 3] = [0x04, 0x01, 0x00];

/// Typed representation of one command byte.
///
/// `Unknown` is intentional: a decoder must be able to inspect a capture
/// without silently assigning a meaning to a byte not present in the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    F1,
    F2,
    F3,
    F5,
    Unknown(u8),
}

impl Command {
    pub const fn from_byte(value: u8) -> Self {
        match value {
            CMD_F1 => Self::F1,
            CMD_F2 => Self::F2,
            CMD_F3 => Self::F3,
            CMD_F5 => Self::F5,
            other => Self::Unknown(other),
        }
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            Self::F1 => CMD_F1,
            Self::F2 => CMD_F2,
            Self::F3 => CMD_F3,
            Self::F5 => CMD_F5,
            Self::Unknown(value) => value,
        }
    }

    pub const fn is_observed(self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
}

impl From<Command> for u8 {
    fn from(value: Command) -> Self {
        value.as_u8()
    }
}

/// Typed representation of a subcommand byte.
///
/// Subcommand numbers overlap between commands, so this type intentionally
/// carries only the byte.  Its meaning is resolved together with the command
/// and report id by [`command_info`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Subcommand(pub u8);

impl Subcommand {
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn as_u8(self) -> u8 {
        self.0
    }
}

impl From<u8> for Subcommand {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<Subcommand> for u8 {
    fn from(value: Subcommand) -> Self {
        value.0
    }
}

/// Why a command is or is not safe for a caller to use.
///
/// The table contains commands observed in the original software, but most
/// parameter layouts remain unverified.  Such commands are deliberately
/// `Unsupported`; this enum has no implicit "write settings" fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCapability {
    /// Observed as a status query/response pair; no write is permitted.
    ReadOnly,
    /// Unknown or observed-but-unverified command/field mapping.
    Unsupported,
    /// A command mapping backed by a reviewed capture record.  The current
    /// table intentionally has no entries with this capability.
    VerifiedWrite,
}

impl CommandCapability {
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::ReadOnly | Self::VerifiedWrite)
    }

    pub const fn can_write(self) -> bool {
        matches!(self, Self::VerifiedWrite)
    }
}

/// Short alias used by UI/apply code.
pub type Capability = CommandCapability;

/// A command header whose write meaning was explicitly reviewed against a
/// concrete capture record.
///
/// This type is deliberately not constructible by callers.  The only
/// production constructor is kept in this module next to the reviewable
/// [`VERIFIED_COMMAND_MAPPINGS`] registry.  A future write mapping therefore
/// has one obvious code-review location and must carry a non-empty evidence
/// reference.  Captured-but-unverified command rows remain [`Unsupported`]
/// and cannot be used to create a [`VerifiedWrite`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VerifiedCommandMapping {
    report_id: u8,
    cmd: u8,
    subcmd: u8,
    frame_len: usize,
    evidence: &'static str,
}

impl VerifiedCommandMapping {
    /// Report ID covered by this reviewed mapping.
    pub const fn report_id(self) -> u8 {
        self.report_id
    }

    /// Command byte covered by this reviewed mapping.
    pub const fn cmd(self) -> u8 {
        self.cmd
    }

    /// Subcommand byte covered by this reviewed mapping.
    pub const fn subcmd(self) -> u8 {
        self.subcmd
    }

    /// Exact full-report length covered by this reviewed mapping.
    pub const fn frame_len(self) -> usize {
        self.frame_len
    }

    /// Human-readable capture/evidence reference retained for review tools.
    pub const fn evidence(self) -> &'static str {
        self.evidence
    }

    /// Build a write token for this exact reviewed mapping.
    ///
    /// The returned token owns a validated frame and is the only frame type
    /// accepted by the gated HID send API.  Raw [`ReportFrame`] values remain
    /// useful for decoding and dry-run inspection but do not confer write
    /// authority.
    #[allow(dead_code)]
    pub(crate) fn frame<P>(self, params: P) -> Result<VerifiedWrite, FrameError>
    where
        P: AsRef<[u8]>,
    {
        let frame = ReportFrame::new(self.report_id, self.cmd, self.subcmd, params)?;
        if frame.expected_len() != self.frame_len {
            return Err(FrameError::InvalidLength {
                actual: frame.expected_len(),
                expected: self.frame_len,
            });
        }
        Ok(VerifiedWrite {
            mapping: self,
            frame,
        })
    }

    /// Construct a mapping from a reviewed capture entry.
    ///
    /// This is intentionally crate-private: adding a production entry must
    /// happen in the registry above the protocol table, where the evidence
    /// reference and exact wire length are visible to reviewers.  The
    /// evidence reference is expected to identify both controlled A/B
    /// persistence observations (including reconnect/readback) and the full
    /// ordered sequence; a correlated one-byte delta is not enough.
    #[allow(dead_code)]
    const fn reviewed(
        report_id: u8,
        cmd: u8,
        subcmd: u8,
        frame_len: usize,
        evidence: &'static str,
    ) -> Self {
        if evidence.is_empty() {
            panic!("verified command mappings require capture evidence");
        }
        if !matches!(
            frame_len,
            REPORT_LEN | EXTENDED_REPORT_LEN | BULK_REPORT_LEN
        ) {
            panic!("verified command mappings require an observed report length");
        }
        Self {
            report_id,
            cmd,
            subcmd,
            frame_len,
            evidence,
        }
    }

    /// Return whether this exact mapping is present in the reviewed registry.
    ///
    /// A crate-local fixture can construct a mapping for protocol tests, but
    /// only an entry registered in [`VERIFIED_COMMAND_MAPPINGS`] is allowed to
    /// reach a device transport.
    pub(crate) fn is_registered(self) -> bool {
        verified_command_info(self.report_id, self.cmd, self.subcmd)
            .is_some_and(|registered| *registered == self)
    }
}

/// A validated frame carrying the explicit write authority of a
/// [`VerifiedCommandMapping`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWrite {
    mapping: VerifiedCommandMapping,
    frame: ReportFrame,
}

impl VerifiedWrite {
    /// Return the reviewed mapping that authorized this frame.
    pub const fn mapping(&self) -> &VerifiedCommandMapping {
        &self.mapping
    }

    /// Return the validated wire frame for inspection.
    pub const fn frame(&self) -> &ReportFrame {
        &self.frame
    }

    /// Return whether this token is backed by the current reviewed registry.
    ///
    /// The token type preserves the mapping, but registry membership is still
    /// checked at every transport/apply boundary so a stale or test-only
    /// fixture cannot become write authority by itself.
    pub(crate) fn is_registry_authorized(&self) -> bool {
        self.mapping.is_registered()
    }

    /// Encode the exact report bytes for a gated HID transfer.
    pub fn as_bytes(&self) -> Result<Vec<u8>, FrameError> {
        self.frame.encode()
    }

    /// Alias for [`Self::as_bytes`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, FrameError> {
        self.as_bytes()
    }
}

/// Errors returned by report construction and decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// The buffer length is not valid for its observed frame kind.
    InvalidLength { actual: usize, expected: usize },
    /// The parameter vector does not match the frame kind selected by its
    /// header.  This is separate from `InvalidLength` so callers can provide
    /// a useful error for a newly assembled frame.
    InvalidParameterLength {
        report_id: u8,
        cmd: u8,
        subcmd: u8,
        actual: usize,
        expected: usize,
    },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { actual, expected } => {
                write!(f, "invalid HID report length {actual}; expected {expected}")
            }
            Self::InvalidParameterLength {
                report_id,
                cmd,
                subcmd,
                actual,
                expected,
            } => write!(
                f,
                "invalid parameter length {actual} for {report_id:02X}/{cmd:02X}/{subcmd:02X}; expected {expected}"
            ),
        }
    }
}

impl std::error::Error for FrameError {}

/// Errors returned while decoding the exact, read-only PollingRate response
/// observed in the official post-Apply readback route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadbackError {
    /// The response length is not the captured 13-byte shape.
    InvalidLength { actual: usize, expected: usize },
    /// A fixed byte in the captured response layout changed unexpectedly.
    UnexpectedByte {
        offset: usize,
        actual: u8,
        expected: u8,
    },
}

impl fmt::Display for ReadbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { actual, expected } => write!(
                f,
                "invalid PollingRate readback length: expected {expected} bytes, got {actual}"
            ),
            Self::UnexpectedByte {
                offset,
                actual,
                expected,
            } => write!(
                f,
                "unexpected PollingRate readback byte at offset {offset}: expected 0x{expected:02X}, got 0x{actual:02X}"
            ),
        }
    }
}

impl std::error::Error for ReadbackError {}

/// A decoded PollingRate response from the observed read-only GET_REPORT
/// route.
///
/// The response is intentionally separate from [`ReportFrame`].  It is a
/// 13-byte device-to-host observation (`02/08/32/...`), not a 16-byte command
/// frame and never confers write authority.  Only the byte at offset 8 varies
/// between the observed rate states; this type does not infer any new mapping
/// from that byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollingRateReadback {
    bytes: [u8; POLLING_RATE_READBACK_LEN],
}

impl PollingRateReadback {
    /// Decode the exact response shape captured after the official Apply.
    pub fn decode(bytes: &[u8]) -> Result<Self, ReadbackError> {
        if bytes.len() != POLLING_RATE_READBACK_LEN {
            return Err(ReadbackError::InvalidLength {
                actual: bytes.len(),
                expected: POLLING_RATE_READBACK_LEN,
            });
        }

        let mut decoded = [0u8; POLLING_RATE_READBACK_LEN];
        decoded.copy_from_slice(bytes);
        for (offset, expected) in [
            (0usize, 0x02u8),
            (1, 0x08),
            (2, 0x32),
            (3, 0x60),
            (4, 0x05),
            (5, 0x00),
            (6, 0xFA),
            (7, 0xFA),
            (9, 0x00),
            (10, 0x08),
            (11, 0x00),
            (12, 0x02),
        ] {
            if decoded[offset] != expected {
                return Err(ReadbackError::UnexpectedByte {
                    offset,
                    actual: decoded[offset],
                    expected,
                });
            }
        }

        Ok(Self { bytes: decoded })
    }

    /// Borrow the exact device response bytes, including the report ID.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the observed wire value at full-response offset 8.
    pub const fn wire_value(&self) -> u8 {
        self.bytes[POLLING_RATE_READBACK_VALUE_OFFSET]
    }

    /// Compare the observation with a caller-supplied, already-authorized
    /// wire value without assigning a new semantic mapping.
    pub const fn matches_wire_value(&self, expected: u8) -> bool {
        self.wire_value() == expected
    }
}

/// Errors returned while decoding the exact selected-DPI response observed in
/// the official post-Apply reconnect/readback route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DpiSelectionReadbackError {
    /// The response length is not the captured 40-byte shape.
    InvalidLength { actual: usize, expected: usize },
    /// A fixed byte in the captured response layout changed unexpectedly.
    UnexpectedByte {
        offset: usize,
        actual: u8,
        expected: u8,
    },
}

impl fmt::Display for DpiSelectionReadbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { actual, expected } => write!(
                f,
                "invalid DPI selection readback length: expected {expected} bytes, got {actual}"
            ),
            Self::UnexpectedByte {
                offset,
                actual,
                expected,
            } => write!(
                f,
                "unexpected DPI selection readback byte at offset {offset}: expected 0x{expected:02X}, got 0x{actual:02X}"
            ),
        }
    }
}

impl std::error::Error for DpiSelectionReadbackError {}

/// A decoded selected-DPI response from the observed read-only GET_REPORT
/// route.
///
/// The response is a 40-byte device-to-host observation beginning with
/// `03 08 42 60 20 ...`.  Controlled A/B evidence shows that full-response
/// byte 8 is the profile `DPI` value for values 1 and 2.  This type remains
/// read-only and never grants write authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpiSelectionReadback {
    bytes: [u8; DPI_SELECTION_READBACK_LEN],
}

const DPI_SELECTION_READBACK_FIXED: [u8; DPI_SELECTION_READBACK_LEN] = [
    0x03, 0x08, 0x42, 0x60, 0x20, 0x00, 0xFA, 0xFA, 0x00, 0x00, 0x01, 0x09, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x21, 0x00, 0x00, 0x00, 0x00, 0x01, 0x3A, 0x00, 0x00, 0x00, 0x00, 0x01, 0x5C, 0x01, 0x00,
    0x00, 0x00, 0x01, 0x7C, 0x04, 0x00, 0x00, 0x00,
];

impl DpiSelectionReadback {
    /// Decode the exact selected-DPI response shape captured after reconnect.
    pub fn decode(bytes: &[u8]) -> Result<Self, DpiSelectionReadbackError> {
        if bytes.len() != DPI_SELECTION_READBACK_LEN {
            return Err(DpiSelectionReadbackError::InvalidLength {
                actual: bytes.len(),
                expected: DPI_SELECTION_READBACK_LEN,
            });
        }

        let mut decoded = [0u8; DPI_SELECTION_READBACK_LEN];
        decoded.copy_from_slice(bytes);
        for (offset, expected) in DPI_SELECTION_READBACK_FIXED.iter().copied().enumerate() {
            if offset == DPI_SELECTION_READBACK_VALUE_OFFSET {
                continue;
            }
            if decoded[offset] != expected {
                return Err(DpiSelectionReadbackError::UnexpectedByte {
                    offset,
                    actual: decoded[offset],
                    expected,
                });
            }
        }

        Ok(Self { bytes: decoded })
    }

    /// Borrow the exact device response bytes, including the report ID.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the selected-DPI value at full-response offset 8.
    pub const fn wire_value(&self) -> u8 {
        self.bytes[DPI_SELECTION_READBACK_VALUE_OFFSET]
    }

    /// Return the selected-DPI value using the profile's `DPI` integer
    /// representation established by the controlled 1-versus-2 A/B capture.
    pub const fn profile_value(&self) -> i32 {
        self.wire_value() as i32
    }

    /// Compare the observation with a profile `DPI` value already covered by
    /// the controlled A/B evidence.
    pub const fn matches_profile_value(&self, expected: i32) -> bool {
        self.profile_value() == expected
    }
}

/// Why a candidate Apply sequence was not authorized.
///
/// A sequence token is intentionally stricter than a command mapping: the
/// physical capture authorizes one complete, ordered burst, not any of the
/// individual reports in that burst.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplySequenceError {
    /// The profile value is not covered by the physical 125 Hz evidence.
    UnsupportedPollingRate { actual: i32, expected: i32 },
    /// An Apply token must contain exactly the observed number of reports.
    InvalidFrameCount { actual: usize, expected: usize },
    /// A candidate report could not be decoded with its exact wire shape.
    MalformedFrame { index: usize, source: FrameError },
    /// A report differs from the captured report at this exact position.
    FrameMismatch { index: usize },
    /// The only permitted substitution has an unverified wire value.
    PollingRateValueMismatch { actual: u8, expected: u8 },
    /// The selected-DPI value is outside the two values covered by the
    /// controlled reconnect/readback A/B capture.
    UnsupportedDpiSelection { actual: i32 },
    /// The only LED mode promoted by the official GUI capture is rainbow
    /// (`LedMode1=2`).
    UnsupportedLedMode { actual: i32 },
}

impl fmt::Display for ApplySequenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPollingRate { actual, expected } => write!(
                f,
                "unsupported profile polling-rate value {actual}; expected audited value {expected}"
            ),
            Self::InvalidFrameCount { actual, expected } => {
                write!(f, "invalid Apply frame count {actual}; expected {expected}")
            }
            Self::MalformedFrame { index, source } => {
                write!(f, "malformed Apply frame at index {index}: {source}")
            }
            Self::FrameMismatch { index } => {
                write!(
                    f,
                    "Apply frame at index {index} does not match the audited sequence"
                )
            }
            Self::PollingRateValueMismatch { actual, expected } => write!(
                f,
                "unsupported PollingRate wire value 0x{actual:02X}; expected 0x{expected:02X}"
            ),
            Self::UnsupportedDpiSelection { actual } => write!(
                f,
                "unsupported DPI profile value {actual}; the reviewed reconnect/readback mapping covers values 1 and 2"
            ),
            Self::UnsupportedLedMode { actual } => write!(
                f,
                "unsupported LED mode profile value {actual}; the reviewed GUI Apply mapping covers rainbow value 2"
            ),
        }
    }
}

impl std::error::Error for ApplySequenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedFrame { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A decoded or to-be-encoded HID feature report.
///
/// `params` contains exactly 13 bytes for ordinary reports and exactly 1021
/// bytes for the observed `04/F3/C8` block report.  The fields are kept as
/// raw bytes at the boundary so unknown captures round-trip without an
/// invented semantic mapping; use [`ReportFrame::command`] and
/// [`ReportFrame::capability`] for typed inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFrame {
    pub report_id: u8,
    pub cmd: u8,
    pub subcmd: u8,
    pub params: Vec<u8>,
}

impl ReportFrame {
    /// Build a report from its four wire-level fields.
    ///
    /// This validates only the exact wire shape.  It does not claim that an
    /// unknown command is safe to send; callers must check [`Self::can_write`]
    /// (which is currently false for every command in this conservative
    /// table) before handing bytes to an I/O layer.
    pub fn new<P>(
        report_id: u8,
        cmd: impl Into<u8>,
        subcmd: impl Into<u8>,
        params: P,
    ) -> Result<Self, FrameError>
    where
        P: AsRef<[u8]>,
    {
        let frame = Self {
            report_id,
            cmd: cmd.into(),
            subcmd: subcmd.into(),
            params: params.as_ref().to_vec(),
        };
        frame.validate_params()?;
        Ok(frame)
    }

    /// Alias for [`Self::new`] that reads naturally at call sites assembling
    /// a raw frame.
    pub fn from_parts<P>(
        report_id: u8,
        cmd: impl Into<u8>,
        subcmd: impl Into<u8>,
        params: P,
    ) -> Result<Self, FrameError>
    where
        P: AsRef<[u8]>,
    {
        Self::new(report_id, cmd, subcmd, params)
    }

    /// Construct the observed 1024-byte block frame.
    pub fn bulk<P>(params: P) -> Result<Self, FrameError>
    where
        P: AsRef<[u8]>,
    {
        Self::new(REPORT_ID_BULK, CMD_F3, SUBCMD_F3_BULK, params)
    }

    /// Decode an exact HID feature-report buffer.
    pub fn decode(bytes: &[u8]) -> Result<Self, FrameError> {
        if bytes.len() < FRAME_HEADER_LEN {
            return Err(FrameError::InvalidLength {
                actual: bytes.len(),
                expected: REPORT_LEN,
            });
        }

        let report_id = bytes[REPORT_ID_OFFSET];
        let cmd = bytes[CMD_OFFSET];
        let subcmd = bytes[SUBCMD_OFFSET];
        let expected = expected_report_len(report_id, cmd, subcmd);
        if bytes.len() != expected {
            return Err(FrameError::InvalidLength {
                actual: bytes.len(),
                expected,
            });
        }

        // `bytes` has already passed the exact length check, so construction
        // cannot fail unless this module's constants are changed
        // inconsistently.
        Self::new(report_id, cmd, subcmd, &bytes[PARAMS_OFFSET..])
    }

    /// Alias for [`Self::decode`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FrameError> {
        Self::decode(bytes)
    }

    /// Validate and encode the frame into the exact buffer passed to HID I/O.
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        self.validate_params()?;
        let mut bytes = Vec::with_capacity(self.expected_len());
        bytes.push(self.report_id);
        bytes.push(self.cmd);
        bytes.push(self.subcmd);
        bytes.extend_from_slice(&self.params);
        debug_assert_eq!(bytes.len(), self.expected_len());
        Ok(bytes)
    }

    /// Alias for [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, FrameError> {
        self.encode()
    }

    /// Check only the exact frame/parameter shape.
    pub fn validate(&self) -> Result<(), FrameError> {
        self.validate_params()
    }

    /// The typed command discriminator.
    pub const fn command(&self) -> Command {
        Command::from_byte(self.cmd)
    }

    /// The typed subcommand byte.
    pub const fn subcommand(&self) -> Subcommand {
        Subcommand(self.subcmd)
    }

    /// Expected encoded size for this frame's header.
    pub const fn expected_len(&self) -> usize {
        expected_report_len(self.report_id, self.cmd, self.subcmd)
    }

    /// Return the observed-table row, if this exact header was observed.
    pub fn known_command(&self) -> Option<&'static KnownCommand> {
        command_info(self.report_id, self.cmd, self.subcmd)
    }

    /// Return the conservative capability for this exact header.
    pub fn capability(&self) -> CommandCapability {
        capability(self.report_id, self.cmd, self.subcmd)
    }

    /// True only for headers present in the disassembly-backed table.
    pub fn is_observed(&self) -> bool {
        self.known_command().is_some()
    }

    /// Whether a caller may use this frame for a device write.
    ///
    /// No write command has a verified field mapping yet, so this is false
    /// for every currently observed command and for unknown commands.
    pub fn can_write(&self) -> bool {
        self.capability().can_write()
    }

    fn validate_params(&self) -> Result<(), FrameError> {
        let expected = expected_param_len(self.report_id, self.cmd, self.subcmd);
        if self.params.len() != expected {
            return Err(FrameError::InvalidParameterLength {
                report_id: self.report_id,
                cmd: self.cmd,
                subcmd: self.subcmd,
                actual: self.params.len(),
                expected,
            });
        }
        Ok(())
    }
}

/// An evidence-backed authorization for one complete physical Apply burst.
///
/// The token contains the exact 156 reports observed during the physical
/// 125 Hz Apply.  Its fields are private so callers cannot turn a standalone
/// `02/F3/32` report, a reordered burst, or a guessed report into an
/// authorization.  The existing [`VerifiedCommandMapping`] registry remains
/// independent and fail-closed; reports exposed by this type intentionally do
/// not make [`ReportFrame::can_write`] return true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedApplySequence {
    frames: Vec<ReportFrame>,
    dpi_selection_profile_value: Option<i32>,
    led_mode_profile_value: Option<i32>,
}

impl VerifiedApplySequence {
    /// Build the reviewed sequence for a profile's polling-rate value.
    ///
    /// Only profile value `8` (the physical 125 Hz capture) is currently
    /// authorized for PollingRate.  The selected-DPI field is left at the
    /// captured baseline value when this compatibility constructor is used.
    pub fn new(profile_polling_rate: i32) -> Result<Self, ApplySequenceError> {
        Self::for_profile_polling_rate(profile_polling_rate)
    }

    /// Build the reviewed sequence for a profile's polling-rate value.
    pub fn for_profile_polling_rate(profile_polling_rate: i32) -> Result<Self, ApplySequenceError> {
        if profile_polling_rate != APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ {
            return Err(ApplySequenceError::UnsupportedPollingRate {
                actual: profile_polling_rate,
                expected: APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ,
            });
        }

        Ok(Self {
            frames: canonical_apply_frames(APPLY_POLLING_RATE_WIRE_VALUE_125_HZ),
            dpi_selection_profile_value: None,
            led_mode_profile_value: None,
        })
    }

    /// Build the reviewed complete sequence for the official GUI's rainbow
    /// LED mode.  The capture authorizes the complete ordered burst with the
    /// `02/F3/49` mode bytes `03 05`; no standalone mode report is emitted.
    pub fn for_profile_polling_rate_and_led_mode(
        profile_polling_rate: i32,
        profile_led_mode: i32,
    ) -> Result<Self, ApplySequenceError> {
        if profile_polling_rate != APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ {
            return Err(ApplySequenceError::UnsupportedPollingRate {
                actual: profile_polling_rate,
                expected: APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ,
            });
        }
        if profile_led_mode != APPLY_LIGHT_MODE_PROFILE_VALUE_RAINBOW {
            return Err(ApplySequenceError::UnsupportedLedMode {
                actual: profile_led_mode,
            });
        }

        Ok(Self {
            frames: canonical_apply_frames(APPLY_POLLING_RATE_WIRE_VALUE_125_HZ),
            dpi_selection_profile_value: None,
            led_mode_profile_value: Some(profile_led_mode),
        })
    }

    /// Build the reviewed complete sequence with a selected-DPI profile value
    /// proven by the controlled A/B reconnect/readback capture.
    ///
    /// Values `1` and `2` are the only selected-DPI values promoted here.  The
    /// complete 156-report order remains mandatory; the selected-DPI byte is
    /// substituted only at report index 16.
    pub fn for_profile_polling_rate_and_dpi(
        profile_polling_rate: i32,
        profile_dpi_selection: i32,
    ) -> Result<Self, ApplySequenceError> {
        if profile_polling_rate != APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ {
            return Err(ApplySequenceError::UnsupportedPollingRate {
                actual: profile_polling_rate,
                expected: APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ,
            });
        }
        let wire_value = match profile_dpi_selection {
            1 | 2 => profile_dpi_selection as u8,
            actual => {
                return Err(ApplySequenceError::UnsupportedDpiSelection { actual });
            }
        };

        Ok(Self {
            frames: canonical_apply_frames_with_dpi(
                APPLY_POLLING_RATE_WIRE_VALUE_125_HZ,
                Some(wire_value),
            ),
            dpi_selection_profile_value: Some(profile_dpi_selection),
            led_mode_profile_value: None,
        })
    }

    /// Build the reviewed complete sequence with both an A/B-proven selected
    /// DPI value and the official GUI rainbow LED mode.
    pub fn for_profile_polling_rate_and_dpi_and_led_mode(
        profile_polling_rate: i32,
        profile_dpi_selection: i32,
        profile_led_mode: i32,
    ) -> Result<Self, ApplySequenceError> {
        if profile_led_mode != APPLY_LIGHT_MODE_PROFILE_VALUE_RAINBOW {
            return Err(ApplySequenceError::UnsupportedLedMode {
                actual: profile_led_mode,
            });
        }
        let mut sequence =
            Self::for_profile_polling_rate_and_dpi(profile_polling_rate, profile_dpi_selection)?;
        sequence.led_mode_profile_value = Some(profile_led_mode);
        Ok(sequence)
    }

    /// Alias using the shorter field name used by the profile model.
    pub fn for_profile_dpi(
        profile_polling_rate: i32,
        profile_dpi_selection: i32,
    ) -> Result<Self, ApplySequenceError> {
        Self::for_profile_polling_rate_and_dpi(profile_polling_rate, profile_dpi_selection)
    }

    /// Alias for callers that use the protocol field name rather than the
    /// profile terminology.
    pub fn for_polling_rate(profile_polling_rate: i32) -> Result<Self, ApplySequenceError> {
        Self::for_profile_polling_rate(profile_polling_rate)
    }

    /// Alias emphasizing that this token is backed by the physical 125 Hz
    /// capture.
    pub fn for_125_hz() -> Result<Self, ApplySequenceError> {
        Self::for_profile_polling_rate(APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ)
    }

    /// Validate a candidate sequence against the complete audited burst.
    ///
    /// This is the only constructor accepting caller-provided frames.  It
    /// checks count, order, exact headers, exact report lengths, every value,
    /// and the one allowed PollingRate position/value.
    pub fn try_from_frames(frames: &[ReportFrame]) -> Result<Self, ApplySequenceError> {
        if frames.len() != APPLY_SEQUENCE_FRAME_COUNT {
            return Err(ApplySequenceError::InvalidFrameCount {
                actual: frames.len(),
                expected: APPLY_SEQUENCE_FRAME_COUNT,
            });
        }

        let variants = [(None, None), (Some(1), Some(1u8)), (Some(2), Some(2u8))];
        for (profile_dpi_selection, wire_value) in variants {
            let expected =
                canonical_apply_frames_with_dpi(APPLY_POLLING_RATE_WIRE_VALUE_125_HZ, wire_value);
            let matches = frames
                .iter()
                .zip(expected.iter())
                .all(|(actual, expected)| {
                    actual
                        .encode()
                        .ok()
                        .zip(expected.encode().ok())
                        .is_some_and(|(actual, expected)| actual == expected)
                });
            if matches {
                return Ok(Self {
                    frames: frames.to_vec(),
                    dpi_selection_profile_value: profile_dpi_selection,
                    led_mode_profile_value: None,
                });
            }
        }

        let expected = canonical_apply_frames(APPLY_POLLING_RATE_WIRE_VALUE_125_HZ);
        for (index, (actual, expected)) in frames.iter().zip(expected.iter()).enumerate() {
            let actual_bytes = actual
                .encode()
                .map_err(|source| ApplySequenceError::MalformedFrame { index, source })?;
            let expected_bytes = expected
                .encode()
                .expect("the evidence sequence must contain valid report shapes");

            if index == APPLY_POLLING_RATE_FRAME_INDEX
                && actual_bytes.len() > POLLING_RATE_OBSERVED_OFFSET
                && actual_bytes[REPORT_ID_OFFSET..=SUBCMD_OFFSET]
                    == [REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_POLLING_RATE_OBSERVED]
                && actual_bytes[POLLING_RATE_OBSERVED_OFFSET]
                    != APPLY_POLLING_RATE_WIRE_VALUE_125_HZ
            {
                return Err(ApplySequenceError::PollingRateValueMismatch {
                    actual: actual_bytes[POLLING_RATE_OBSERVED_OFFSET],
                    expected: APPLY_POLLING_RATE_WIRE_VALUE_125_HZ,
                });
            }

            if index == APPLY_DPI_SELECTION_FRAME_INDEX
                && actual_bytes.len() > DPI_SELECTION_APPLY_OBSERVED_OFFSET
                && actual_bytes[REPORT_ID_OFFSET..=SUBCMD_OFFSET]
                    == [REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_42]
            {
                let actual = actual_bytes[DPI_SELECTION_APPLY_OBSERVED_OFFSET] as i32;
                if !AUTHORIZED_DPI_SELECTION_PROFILE_VALUES.contains(&actual) {
                    return Err(ApplySequenceError::UnsupportedDpiSelection { actual });
                }
            }

            if actual_bytes != expected_bytes {
                return Err(ApplySequenceError::FrameMismatch { index });
            }
        }

        Ok(Self {
            frames: frames.to_vec(),
            dpi_selection_profile_value: None,
            led_mode_profile_value: None,
        })
    }

    /// Alias for [`Self::try_from_frames`] using authorization terminology.
    pub fn authorize(frames: &[ReportFrame]) -> Result<Self, ApplySequenceError> {
        Self::try_from_frames(frames)
    }

    /// Decode and validate candidate wire reports as one complete sequence.
    pub fn try_from_bytes<B>(reports: &[B]) -> Result<Self, ApplySequenceError>
    where
        B: AsRef<[u8]>,
    {
        if reports.len() != APPLY_SEQUENCE_FRAME_COUNT {
            return Err(ApplySequenceError::InvalidFrameCount {
                actual: reports.len(),
                expected: APPLY_SEQUENCE_FRAME_COUNT,
            });
        }

        let frames = reports
            .iter()
            .enumerate()
            .map(|(index, report)| {
                ReportFrame::decode(report.as_ref())
                    .map_err(|source| ApplySequenceError::MalformedFrame { index, source })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_from_frames(&frames)
    }

    /// Alias for [`Self::try_from_bytes`].
    pub fn from_bytes<B>(reports: &[B]) -> Result<Self, ApplySequenceError>
    where
        B: AsRef<[u8]>,
    {
        Self::try_from_bytes(reports)
    }

    /// Borrow the exact ordered reports for the apply worker.
    pub fn frames(&self) -> &[ReportFrame] {
        &self.frames
    }

    /// Borrow one exact report by its zero-based sequence index.
    pub fn frame(&self, index: usize) -> Option<&ReportFrame> {
        self.frames.get(index)
    }

    /// Iterate over the exact ordered reports for the apply worker.
    pub fn iter(&self) -> impl Iterator<Item = &ReportFrame> {
        self.frames.iter()
    }

    /// Number of reports in this authorization token.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// This token is never empty; provided for collection-like call sites.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Return the authorized profile polling-rate value.
    pub const fn profile_polling_rate(&self) -> i32 {
        APPLY_POLLING_RATE_PROFILE_VALUE_125_HZ
    }

    /// Return the authorized wire PollingRate byte at report index 13.
    pub const fn polling_rate_wire_value(&self) -> u8 {
        APPLY_POLLING_RATE_WIRE_VALUE_125_HZ
    }

    /// Return the selected-DPI profile value when this token carries one of
    /// the promoted A/B-proven substitutions.
    pub const fn profile_dpi_selection(&self) -> Option<i32> {
        self.dpi_selection_profile_value
    }

    /// Return the promoted profile LED mode when this token was built for the
    /// official GUI rainbow mapping.
    pub const fn profile_led_mode(&self) -> Option<i32> {
        self.led_mode_profile_value
    }

    /// Return the selected-DPI wire byte when this token carries one of the
    /// promoted A/B-proven substitutions.
    pub const fn dpi_selection_wire_value(&self) -> Option<u8> {
        match self.dpi_selection_profile_value {
            Some(value) => Some(value as u8),
            None => None,
        }
    }

    /// Return the evidence reference for this token.
    pub const fn evidence(&self) -> &'static str {
        if self.led_mode_profile_value.is_some() {
            LIGHT_MODE_RAINBOW_EVIDENCE
        } else {
            match self.dpi_selection_profile_value {
                Some(_) => DPI_SELECTION_EVIDENCE,
                None => APPLY_SEQUENCE_EVIDENCE,
            }
        }
    }

    /// Encode the exact ordered reports for a gated transport adapter.
    pub fn as_bytes(&self) -> Result<Vec<Vec<u8>>, FrameError> {
        self.frames.iter().map(ReportFrame::encode).collect()
    }

    /// Alias for [`Self::as_bytes`].
    pub fn to_bytes(&self) -> Result<Vec<Vec<u8>>, FrameError> {
        self.as_bytes()
    }
}

impl AsRef<[ReportFrame]> for VerifiedApplySequence {
    fn as_ref(&self) -> &[ReportFrame] {
        self.frames()
    }
}

impl<'a> IntoIterator for &'a VerifiedApplySequence {
    type Item = &'a ReportFrame;
    type IntoIter = std::slice::Iter<'a, ReportFrame>;

    fn into_iter(self) -> Self::IntoIter {
        self.frames.iter()
    }
}

/// Alias for callers that name the token by its authorization role.
pub type ApplySequenceAuthorization = VerifiedApplySequence;

/// Reconstruct the 16-byte report that carried the only controlled
/// polling-rate delta in the paired USBPcap2 captures.
///
/// `wire_value` is deliberately supplied by the caller because the paired
/// transition captures establish `0x04` and `0x01`, while the physical 125 Hz
/// Apply establishes `0x08`.  This helper is useful for evidence tests and
/// byte-level comparison; [`ReportFrame::can_write`] remains false and the
/// apply planner will not send it as a standalone report.
pub fn observed_polling_transition_frame(wire_value: u8) -> Result<ReportFrame, FrameError> {
    let mut params = [0u8; PARAM_LEN];
    params[1] = 0x06;
    params[5] = wire_value;
    params[7] = 0x08;
    params[9] = 0x02;
    ReportFrame::new(
        REPORT_ID_CONFIG,
        CMD_F3,
        SUBCMD_F3_POLLING_RATE_OBSERVED,
        params,
    )
}

/// Names commonly used by callers integrating a HID layer.
pub type FeatureReport = ReportFrame;
pub type HidReport = ReportFrame;
pub type Report = ReportFrame;
pub type ProtocolError = FrameError;

/// One observed report header and its conservative capability classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownCommand {
    pub report_id: u8,
    pub cmd: u8,
    pub subcmd: u8,
    pub frame_len: usize,
    pub capability: CommandCapability,
}

impl KnownCommand {
    pub const fn command(self) -> Command {
        Command::from_byte(self.cmd)
    }

    pub const fn subcommand(self) -> Subcommand {
        Subcommand(self.subcmd)
    }

    pub fn can_write(self) -> bool {
        can_write(self.report_id, self.cmd, self.subcmd)
    }

    pub fn is_supported(self) -> bool {
        capability(self.report_id, self.cmd, self.subcmd).is_supported()
    }
}

const fn standard_command(report_id: u8, cmd: u8, subcmd: u8) -> KnownCommand {
    KnownCommand {
        report_id,
        cmd,
        subcmd,
        frame_len: REPORT_LEN,
        capability: CommandCapability::Unsupported,
    }
}

const fn extended_command(report_id: u8, cmd: u8, subcmd: u8) -> KnownCommand {
    KnownCommand {
        report_id,
        cmd,
        subcmd,
        frame_len: EXTENDED_REPORT_LEN,
        capability: CommandCapability::Unsupported,
    }
}

const fn read_only_command(report_id: u8, cmd: u8, subcmd: u8) -> KnownCommand {
    KnownCommand {
        report_id,
        cmd,
        subcmd,
        frame_len: REPORT_LEN,
        capability: CommandCapability::ReadOnly,
    }
}

const fn bulk_command(report_id: u8, cmd: u8, subcmd: u8) -> KnownCommand {
    KnownCommand {
        report_id,
        cmd,
        subcmd,
        frame_len: BULK_REPORT_LEN,
        capability: CommandCapability::Unsupported,
    }
}

/// Conservative table of exact command headers observed in `ANALYSIS.md`.
///
/// The F1/F5 rows intentionally include each observed subcommand under each
/// command because the notes record the command family and the subcommand set
/// but do not establish a safe one-to-one pairing.  Every non-status entry is
/// unsupported until a capture verifies its parameter layout and effect.
pub const KNOWN_COMMANDS: &[KnownCommand] = &[
    // F2: report 02 status/query and configuration-family observations.
    read_only_command(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_STATUS),
    standard_command(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_04),
    standard_command(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_20),
    standard_command(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_32),
    standard_command(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_38),
    standard_command(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_3D),
    // F2: report 03 observations.
    standard_command(REPORT_ID_CONFIG_EXTENDED, CMD_F2, SUBCMD_F2_20),
    standard_command(REPORT_ID_CONFIG_EXTENDED, CMD_F2, SUBCMD_F2_32),
    // F3: report 02/03 configuration-family observations.
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_20),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_2C),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_32),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_38),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_42),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_44),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_46),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_49),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_4F),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_5C),
    standard_command(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_8E),
    // The controlled USBPcap2 trace observed this exact header with a
    // 64-byte report.  Other report-03 rows stay at the conservative
    // standard length until an equivalent capture establishes their shape.
    extended_command(REPORT_ID_CONFIG_EXTENDED, CMD_F3, SUBCMD_F3_20),
    standard_command(REPORT_ID_CONFIG_EXTENDED, CMD_F3, SUBCMD_F3_2C),
    standard_command(REPORT_ID_CONFIG_EXTENDED, CMD_F3, SUBCMD_F3_32),
    standard_command(REPORT_ID_CONFIG_EXTENDED, CMD_F3, SUBCMD_F3_38),
    standard_command(REPORT_ID_CONFIG_EXTENDED, CMD_F3, SUBCMD_F3_46),
    // F3/20 on report 03 is the observed 64-byte report; F3/C8 remains the
    // separate disassembly-backed 1024-byte candidate.
    bulk_command(REPORT_ID_BULK, CMD_F3, SUBCMD_F3_BULK),
    // F1/F5 commit/reset-family observations.  Pairing is unresolved.
    standard_command(REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_01),
    standard_command(REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_02),
    standard_command(REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_04),
    standard_command(REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_08),
    standard_command(REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_10),
    standard_command(REPORT_ID_CONFIG, CMD_F5, SUBCMD_F5_00),
    standard_command(REPORT_ID_CONFIG, CMD_F5, SUBCMD_F5_01),
    standard_command(REPORT_ID_CONFIG, CMD_F5, SUBCMD_F5_04),
    standard_command(REPORT_ID_CONFIG, CMD_F5, SUBCMD_F5_08),
];

/// Alias retained for callers that prefer an explicit table name.
pub const COMMAND_TABLE: &[KnownCommand] = KNOWN_COMMANDS;
pub const KNOWN_COMMAND_TABLE: &[KnownCommand] = KNOWN_COMMANDS;

/// Explicit write mappings reviewed against concrete capture evidence.
///
/// This registry is intentionally empty until a complete command mapping is
/// established.  Observed command rows above are not write permission; a
/// future entry must include an exact header, report length, and evidence
/// reference in the same reviewable diff.  In particular, do not add an
/// entry from a correlated A/B delta alone: exact A/B persistence after
/// reconnect/readback and the complete ordered apply sequence are required.
pub const VERIFIED_COMMAND_MAPPINGS: &[VerifiedCommandMapping] = &[];

/// Borrow the currently reviewed write mappings.
pub fn verified_command_mappings() -> &'static [VerifiedCommandMapping] {
    VERIFIED_COMMAND_MAPPINGS
}

/// Find a reviewed write mapping for an exact observed header.
pub fn verified_command_info(
    report_id: u8,
    cmd: u8,
    subcmd: u8,
) -> Option<&'static VerifiedCommandMapping> {
    VERIFIED_COMMAND_MAPPINGS.iter().find(|mapping| {
        mapping.report_id == report_id
            && mapping.cmd == cmd
            && mapping.subcmd == subcmd
            && command_info(report_id, cmd, subcmd)
                .is_some_and(|observed| observed.frame_len == mapping.frame_len)
    })
}

/// Find the exact observed command row for a header.
pub fn command_info(report_id: u8, cmd: u8, subcmd: u8) -> Option<&'static KnownCommand> {
    KNOWN_COMMANDS
        .iter()
        .find(|entry| entry.report_id == report_id && entry.cmd == cmd && entry.subcmd == subcmd)
}

/// Whether the exact header was observed, independent of whether its fields
/// are currently safe to use.
pub fn is_observed_command(report_id: u8, cmd: u8, subcmd: u8) -> bool {
    command_info(report_id, cmd, subcmd).is_some()
}

/// Return the conservative capability for an exact header.
///
/// `VerifiedWrite` is returned only when the header is present in the
/// evidence-backed [`VERIFIED_COMMAND_MAPPINGS`] registry; an observed row
/// that is absent from that registry remains `Unsupported`.
pub fn capability(report_id: u8, cmd: u8, subcmd: u8) -> CommandCapability {
    let Some(entry) = command_info(report_id, cmd, subcmd) else {
        return CommandCapability::Unsupported;
    };
    if entry.capability == CommandCapability::ReadOnly {
        return CommandCapability::ReadOnly;
    }
    if verified_command_info(report_id, cmd, subcmd).is_some() {
        return CommandCapability::VerifiedWrite;
    }
    entry.capability
}

/// Explicit deny-by-default write check for apply code.
pub fn can_write(report_id: u8, cmd: u8, subcmd: u8) -> bool {
    capability(report_id, cmd, subcmd).can_write()
}

/// Alias for [`is_observed_command`].
pub fn is_known_command(report_id: u8, cmd: u8, subcmd: u8) -> bool {
    is_observed_command(report_id, cmd, subcmd)
}

#[cfg(test)]
pub(crate) fn test_verified_command_mapping(
    report_id: u8,
    cmd: u8,
    subcmd: u8,
    frame_len: usize,
) -> VerifiedCommandMapping {
    VerifiedCommandMapping::reviewed(
        report_id,
        cmd,
        subcmd,
        frame_len,
        "unit-test capture fixture",
    )
}

/// Exact wire reports from the physical 125 Hz Apply burst.  The sole
/// variable field is retained at report index 13 and is substituted by
/// [`canonical_apply_frames`] so the authorized value is explicit at the
/// construction site.
const PHYSICAL_125HZ_APPLY_REPORTS: [&str; APPLY_SEQUENCE_FRAME_COUNT] = [
    "02f50000000000000000000000000000", // 000
    "02f33e00020000000000000000000000", // 001
    "02f34604020000000000000000000000", // 002
    "02f3490406000000ff00000305010000", // 003
    "02f34f04010000000200000000000000", // 004
    "02f35104060000000000ff0305010000", // 005
    "02f35704010000000200000000000000", // 006
    "02f359040600000000ff000305010000", // 007
    "02f35f04010000000200000000000000", // 008
    "02f3610406000000ff00ff0305010000", // 009
    "02f36704010000000200000000000000", // 010
    "02f3690406000000ffff000305010000", // 011
    "02f36f04010000000200000000000000", // 012
    "02f33200060000000800080002000000", // 013
    "02f33800040000000200020000000000", // 014
    concat!(
        "03f320000a000000",
        "0100010001000100",
        "0100000000000000",
        "0000000000000000",
        "0000000000000000",
        "0000000000000000",
        "0000000000000000",
        "0000000000000000",
    ), // 015
    "02f34200020000000000000000000000", // 016
    "02f30201020000000000000000000000", // 017
    "02f3b201020000000000000000000000", // 018
    "02f36202020000000100000000000000", // 019
    "02f31203020000000100000000000000", // 020
    "02f10202000000000000000000000000", // 021
    "02f10210000000000000000000000000", // 022
    "02f34400050000000116000000000000", // 023
    "02f30401050000000116000000000000", // 024
    "02f3b401050000000116000000000000", // 025
    "02f36402050000000116000000000000", // 026
    "02f31403050000000116000000000000", // 027
    "02f34a0005000000012d000000000000", // 028
    "02f30a0105000000012d000000000000", // 029
    "02f3ba0105000000012d000000000000", // 030
    "02f36a0205000000012d000000000000", // 031
    "02f31a0305000000012d000000000000", // 032
    "02f3500005000000015a000000000000", // 033
    "02f3100105000000015a000000000000", // 034
    "02f3c00105000000015a000000000000", // 035
    "02f3700205000000015a000000000000", // 036
    "02f3200305000000015a000000000000", // 037
    "02f3560005000000015c010000000000", // 038
    "02f3160105000000015c010000000000", // 039
    "02f3c60105000000015c010000000000", // 040
    "02f3760205000000015c010000000000", // 041
    "02f3260305000000015c010000000000", // 042
    "02f35c0005000000017c040000000000", // 043
    "02f31c0105000000017c040000000000", // 044
    "02f3cc0105000000017c040000000000", // 045
    "02f37c0205000000017c040000000000", // 046
    "02f32c0305000000017c040000000000", // 047
    "02f10210000000000000000000000000", // 048
    "02f32c00020000000000000000000000", // 049
    "02f10201000000000000000000000000", // 050
    "02f38200040000008100000000000000", // 051
    "02f38600040000008200000000000000", // 052
    "02f38a00040000008300000000000000", // 053
    "02f38e00040000008400000000000000", // 054
    "02f39200040000008300000000000000", // 055
    "02f39600040000008300000000000000", // 056
    "02f39a000400000090001e0000000000", // 057
    "02f39e000400000090001f0000000000", // 058
    "02f3a200040000009000200000000000", // 059
    "02f3a600040000009000210000000000", // 060
    "02f3aa00040000009000220000000000", // 061
    "02f3ae00040000009000230000000000", // 062
    "02f3b200040000009000240000000000", // 063
    "02f3b600040000009000250000000000", // 064
    "02f3ba00040000009000260000000000", // 065
    "02f3be00040000009000270000000000", // 066
    "02f3c2000400000090002d0000000000", // 067
    "02f3c600040000009000340000000000", // 068
    "02f3da00040000008b00000000000000", // 069
    "02f3de00040000008c00000000000000", // 070
    "02f34201040000008100000000000000", // 071
    "02f34601040000008200000000000000", // 072
    "02f34a01040000008300000000000000", // 073
    "02f34e01040000008400000000000000", // 074
    "02f35201040000009000380000000000", // 075
    "02f35601040000008900000000000000", // 076
    "02f35a010400000090001e0000000000", // 077
    "02f35e010400000090001f0000000000", // 078
    "02f36201040000009000200000000000", // 079
    "02f36601040000009000210000000000", // 080
    "02f36a01040000009000220000000000", // 081
    "02f36e01040000009000230000000000", // 082
    "02f37201040000009000240000000000", // 083
    "02f37601040000009000250000000000", // 084
    "02f37a01040000009000260000000000", // 085
    "02f37e01040000009000270000000000", // 086
    "02f382010400000090002d0000000000", // 087
    "02f386010400000090002f0000000000", // 088
    "02f39a01040000008b00000000000000", // 089
    "02f39e01040000008c00000000000000", // 090
    "02f3f201040000008100000000000000", // 091
    "02f3f601040000008200000000000000", // 092
    "02f3fa01040000008300000000000000", // 093
    "02f3fe01040000009981010000000000", // 094
    "02f30202040000008a00000000000000", // 095
    "02f30602040000008900000000000000", // 096
    "02f30a02040000009100010000000000", // 097
    "02f30e02040000009101010000000000", // 098
    "02f31202040000009000200000000000", // 099
    "02f31602040000009000210000000000", // 100
    "02f31a02040000009000220000000000", // 101
    "02f31e02040000009000230000000000", // 102
    "02f32202040000009000240000000000", // 103
    "02f32602040000009000250000000000", // 104
    "02f32a02040000009000260000000000", // 105
    "02f32e02040000009000270000000000", // 106
    "02f33202040000009000570000000000", // 107
    "02f33602040000009000560000000000", // 108
    "02f34a02040000008b00000000000000", // 109
    "02f34e02040000008c00000000000000", // 110
    "02f3a202040000008100000000000000", // 111
    "02f3a602040000008200000000000000", // 112
    "02f3aa02040000008300000000000000", // 113
    "02f3ae02040000009981030000000000", // 114
    "02f3b202040000008a00000000000000", // 115
    "02f3b602040000008900000000000000", // 116
    "02f3ba020400000090001e0000000000", // 117
    "02f3be020400000090001f0000000000", // 118
    "02f3c202040000009000200000000000", // 119
    "02f3c602040000009000210000000000", // 120
    "02f3ca02040000009000220000000000", // 121
    "02f3ce02040000009000230000000000", // 122
    "02f3d202040000009000240000000000", // 123
    "02f3d602040000009000250000000000", // 124
    "02f3da02040000009000260000000000", // 125
    "02f3de02040000009000270000000000", // 126
    "02f3e202040000009000570000000000", // 127
    "02f3e602040000009000560000000000", // 128
    "02f3fa02040000008b00000000000000", // 129
    "02f3fe02040000008c00000000000000", // 130
    "02f35203040000008100000000000000", // 131
    "02f35603040000008200000000000000", // 132
    "02f35a03040000008300000000000000", // 133
    "02f35e03040000009981030000000000", // 134
    "02f36203040000008a00000000000000", // 135
    "02f36603040000008900000000000000", // 136
    "02f36a030400000090001e0000000000", // 137
    "02f36e030400000090001f0000000000", // 138
    "02f37203040000009000200000000000", // 139
    "02f37603040000009000210000000000", // 140
    "02f37a03040000009000220000000000", // 141
    "02f37e03040000009000230000000000", // 142
    "02f38203040000009000240000000000", // 143
    "02f38603040000009000250000000000", // 144
    "02f38a03040000009000260000000000", // 145
    "02f38e03040000009000270000000000", // 146
    "02f39203040000009000570000000000", // 147
    "02f39603040000009000560000000000", // 148
    "02f3aa03040000008b00000000000000", // 149
    "02f3ae03040000008c00000000000000", // 150
    "02f10204000000000000000000000000", // 151
    "02f10201000000000000000000000000", // 152
    "02f10202000000000000000000000000", // 153
    "02f10208000000000000000000000000", // 154
    "02f50100000000000000000000000000", // 155
];

fn canonical_apply_frames(polling_rate_wire_value: u8) -> Vec<ReportFrame> {
    canonical_apply_frames_with_dpi(polling_rate_wire_value, None)
}

fn canonical_apply_frames_with_dpi(
    polling_rate_wire_value: u8,
    dpi_selection_wire_value: Option<u8>,
) -> Vec<ReportFrame> {
    PHYSICAL_125HZ_APPLY_REPORTS
        .iter()
        .enumerate()
        .map(|(index, encoded)| {
            let mut bytes = decode_hex_report(encoded);
            if index == APPLY_POLLING_RATE_FRAME_INDEX {
                bytes[POLLING_RATE_OBSERVED_OFFSET] = polling_rate_wire_value;
            }
            if index == APPLY_DPI_SELECTION_FRAME_INDEX {
                if let Some(value) = dpi_selection_wire_value {
                    bytes[DPI_SELECTION_APPLY_OBSERVED_OFFSET] = value;
                }
            }
            ReportFrame::decode(&bytes).unwrap_or_else(|source| {
                panic!("invalid captured Apply frame at index {index}: {source}")
            })
        })
        .collect()
}

fn decode_hex_report(encoded: &str) -> Vec<u8> {
    assert!(
        encoded.len() % 2 == 0,
        "captured Apply report must contain complete byte pairs"
    );
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0]).expect("captured Apply report must be hexadecimal");
            let low = hex_nibble(pair[1]).expect("captured Apply report must be hexadecimal");
            (high << 4) | low
        })
        .collect()
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

const fn is_bulk_header(report_id: u8, cmd: u8, subcmd: u8) -> bool {
    report_id == REPORT_ID_BULK && cmd == CMD_F3 && subcmd == SUBCMD_F3_BULK
}

const fn expected_report_len(report_id: u8, cmd: u8, subcmd: u8) -> usize {
    if is_bulk_header(report_id, cmd, subcmd) {
        BULK_REPORT_LEN
    } else if is_extended_header(report_id, cmd, subcmd) {
        EXTENDED_REPORT_LEN
    } else {
        REPORT_LEN
    }
}

const fn is_extended_header(report_id: u8, cmd: u8, subcmd: u8) -> bool {
    report_id == REPORT_ID_CONFIG_EXTENDED && cmd == CMD_F3 && subcmd == SUBCMD_F3_20
}

const fn expected_param_len(report_id: u8, cmd: u8, subcmd: u8) -> usize {
    expected_report_len(report_id, cmd, subcmd) - FRAME_HEADER_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_frame_round_trips_exactly() {
        let params = [0x01, 0x00, 0xAA, 0x55, 0, 1, 2, 3, 4, 5, 6, 7, 8];
        let frame = ReportFrame::new(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_STATUS, params)
            .expect("observed standard frame");
        let bytes = frame.encode().expect("encode");
        assert_eq!(bytes.len(), REPORT_LEN);
        assert_eq!(&bytes[..3], &[REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_STATUS]);
        assert_eq!(ReportFrame::decode(&bytes).expect("decode"), frame);
    }

    #[test]
    fn bulk_frame_round_trips_exactly() {
        let params = vec![0xA5; BULK_REPORT_LEN - FRAME_HEADER_LEN];
        let frame = ReportFrame::new(REPORT_ID_BULK, CMD_F3, SUBCMD_F3_BULK, params.clone())
            .expect("observed bulk frame");
        let bytes = frame.encode().expect("encode");
        assert_eq!(bytes.len(), BULK_REPORT_LEN);
        assert_eq!(&bytes[..3], &[REPORT_ID_BULK, CMD_F3, SUBCMD_F3_BULK]);
        assert_eq!(&bytes[3..], params.as_slice());
        assert_eq!(ReportFrame::decode(&bytes).expect("decode"), frame);
    }

    #[test]
    fn observed_extended_frame_round_trips_at_sixty_four_bytes() {
        let params = vec![0x01; EXTENDED_PARAM_LEN];
        let frame = ReportFrame::new(REPORT_ID_CONFIG_EXTENDED, CMD_F3, SUBCMD_F3_20, params)
            .expect("observed extended frame");
        let bytes = frame.encode().expect("encode");
        assert_eq!(bytes.len(), EXTENDED_REPORT_LEN);
        assert_eq!(ReportFrame::decode(&bytes).expect("decode"), frame);
        assert!(
            !frame.can_write(),
            "capture evidence is not write authorization"
        );
    }

    #[test]
    fn polling_transition_fixture_matches_the_captured_layout() {
        let frame = observed_polling_transition_frame(0x04).expect("fixture");
        assert_eq!(
            frame.encode().expect("encode"),
            [
                0x02, 0xF3, 0x32, 0x00, 0x06, 0x00, 0x00, 0x00, 0x04, 0x00, 0x08, 0x00, 0x02, 0x00,
                0x00, 0x00,
            ]
        );
        assert!(!frame.can_write());
    }

    #[test]
    fn invalid_lengths_are_rejected() {
        assert!(matches!(
            ReportFrame::decode(&[0; REPORT_LEN - 1]),
            Err(FrameError::InvalidLength { .. })
        ));
        assert!(matches!(
            ReportFrame::decode(&[0; REPORT_LEN + 1]),
            Err(FrameError::InvalidLength { .. })
        ));

        let mut wrong_bulk = vec![0; REPORT_LEN];
        wrong_bulk[..3].copy_from_slice(&[REPORT_ID_BULK, CMD_F3, SUBCMD_F3_BULK]);
        assert!(matches!(
            ReportFrame::decode(&wrong_bulk),
            Err(FrameError::InvalidLength { .. })
        ));

        let mut wrong_bulk_len = vec![0; BULK_REPORT_LEN - 1];
        wrong_bulk_len[..3].copy_from_slice(&[REPORT_ID_BULK, CMD_F3, SUBCMD_F3_BULK]);
        assert!(matches!(
            ReportFrame::decode(&wrong_bulk_len),
            Err(FrameError::InvalidLength { .. })
        ));
    }

    #[test]
    fn observed_command_table_contains_analysis_frames() {
        for entry in KNOWN_COMMANDS {
            assert!(is_observed_command(
                entry.report_id,
                entry.cmd,
                entry.subcmd
            ));
            assert_eq!(
                command_info(entry.report_id, entry.cmd, entry.subcmd),
                Some(entry)
            );
        }
        assert_eq!(
            command_info(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_STATUS)
                .unwrap()
                .frame_len,
            REPORT_LEN
        );
        assert_eq!(
            command_info(REPORT_ID_BULK, CMD_F3, SUBCMD_F3_BULK)
                .unwrap()
                .frame_len,
            BULK_REPORT_LEN
        );
        assert_eq!(
            command_info(REPORT_ID_CONFIG_EXTENDED, CMD_F3, SUBCMD_F3_20)
                .unwrap()
                .frame_len,
            EXTENDED_REPORT_LEN
        );
        for subcmd in [
            SUBCMD_F3_DPI_VALUE_OBSERVED,
            SUBCMD_F3_LIGHT_MODE_COLOR_OBSERVED,
            SUBCMD_F3_LIGHT_BRIGHTNESS_OBSERVED,
            SUBCMD_F3_DPI_ENABLE_OBSERVED,
            SUBCMD_F3_BUTTON_FUNCTION_OBSERVED,
        ] {
            assert!(command_info(REPORT_ID_CONFIG, CMD_F3, subcmd).is_some());
        }
        for (cmd, subcmd) in [
            (CMD_F1, SUBCMD_F1_02),
            (CMD_F1, SUBCMD_F1_10),
            (CMD_F5, SUBCMD_F5_00),
            (CMD_F5, SUBCMD_F5_01),
        ] {
            assert!(command_info(REPORT_ID_CONFIG, cmd, subcmd).is_some());
        }
    }

    #[test]
    fn unknown_and_unverified_commands_are_unsupported() {
        assert_eq!(
            capability(REPORT_ID_CONFIG, 0xEE, 0x01),
            CommandCapability::Unsupported
        );
        assert_eq!(
            capability(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_STATUS),
            CommandCapability::ReadOnly
        );
        assert!(!can_write(REPORT_ID_CONFIG, CMD_F2, SUBCMD_F2_STATUS));
        assert!(KNOWN_COMMANDS.iter().all(|entry| !entry.can_write()));

        let frame = ReportFrame::new(REPORT_ID_CONFIG, 0xEE, 0x01, [0; PARAM_LEN]).unwrap();
        assert_eq!(frame.capability(), CommandCapability::Unsupported);
        assert!(!frame.can_write());
    }

    #[test]
    fn verified_write_registry_stays_empty_without_capture_evidence() {
        assert!(VERIFIED_COMMAND_MAPPINGS.is_empty());
        assert!(verified_command_mappings().is_empty());
        assert!(!can_write(REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_01));
    }

    #[test]
    fn unregistered_reviewed_fixture_has_no_write_authority() {
        let mapping =
            test_verified_command_mapping(REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_01, REPORT_LEN);
        let write = mapping
            .frame([0xA5; PARAM_LEN])
            .expect("fixture still validates its wire shape");

        assert!(!mapping.is_registered());
        assert!(!write.is_registry_authorized());
    }

    #[test]
    fn reviewed_mapping_builds_an_exact_verified_write_fixture() {
        let mapping =
            test_verified_command_mapping(REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_01, REPORT_LEN);
        let write = mapping
            .frame([0xA5; PARAM_LEN])
            .expect("exact mapped parameters");

        assert_eq!(write.mapping(), &mapping);
        assert_eq!(write.as_bytes().expect("encode").len(), REPORT_LEN);
        assert_eq!(
            write.as_bytes().expect("encode")[..3],
            [REPORT_ID_CONFIG, CMD_F1, SUBCMD_F1_01,]
        );
    }

    #[test]
    fn physical_125hz_apply_sequence_is_exactly_156_frames() {
        let sequence = VerifiedApplySequence::for_profile_polling_rate(8)
            .expect("the audited 125 Hz profile value is authorized");

        assert_eq!(sequence.len(), APPLY_SEQUENCE_FRAME_COUNT);
        assert_eq!(
            sequence.frames()[0].encode().expect("encode"),
            hex_bytes("02f50000000000000000000000000000")
        );
        assert_eq!(
            sequence.frames()[APPLY_POLLING_RATE_FRAME_INDEX]
                .encode()
                .expect("encode"),
            hex_bytes("02f33200060000000800080002000000")
        );
        assert_eq!(sequence.frames()[15].expected_len(), EXTENDED_REPORT_LEN);
        assert_eq!(
            sequence.frames()[APPLY_SEQUENCE_FRAME_COUNT - 1]
                .encode()
                .expect("encode"),
            hex_bytes("02f50100000000000000000000000000")
        );
        assert!(sequence.frames().iter().all(|frame| !frame.can_write()));
    }

    #[test]
    fn apply_sequence_authorizer_rejects_standalone_polling_frame_and_mutations() {
        let authorized = VerifiedApplySequence::for_profile_polling_rate(8)
            .expect("the audited 125 Hz profile value is authorized");

        assert!(VerifiedApplySequence::try_from_frames(&[authorized.frames()
            [APPLY_POLLING_RATE_FRAME_INDEX]
            .clone()])
        .is_err());

        let mut wrong_order = authorized.frames().to_vec();
        wrong_order.swap(0, 1);
        assert!(VerifiedApplySequence::try_from_frames(&wrong_order).is_err());

        let mut wrong_header = authorized.frames().to_vec();
        wrong_header[APPLY_POLLING_RATE_FRAME_INDEX].subcmd = SUBCMD_F3_38;
        assert!(VerifiedApplySequence::try_from_frames(&wrong_header).is_err());

        let mut wrong_value = authorized.frames().to_vec();
        wrong_value[APPLY_POLLING_RATE_FRAME_INDEX].params
            [POLLING_RATE_OBSERVED_OFFSET - PARAMS_OFFSET] = 0x04;
        assert!(VerifiedApplySequence::try_from_frames(&wrong_value).is_err());

        let mut wrong_length = authorized.frames().to_vec();
        wrong_length[15].params.pop();
        assert!(VerifiedApplySequence::try_from_frames(&wrong_length).is_err());
    }

    #[test]
    fn only_the_audited_125hz_profile_value_can_build_an_apply_sequence() {
        for profile_value in [1, 2, 4, 0, 9] {
            assert!(VerifiedApplySequence::for_profile_polling_rate(profile_value).is_err());
        }
        assert!(VerifiedApplySequence::for_profile_polling_rate(8).is_ok());
        assert!(!can_write(REPORT_ID_CONFIG, CMD_F3, SUBCMD_F3_32));
        assert!(VERIFIED_COMMAND_MAPPINGS.is_empty());
    }

    fn hex_bytes(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0);
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let high = (pair[0] as char).to_digit(16).expect("hex") as u8;
                let low = (pair[1] as char).to_digit(16).expect("hex") as u8;
                (high << 4) | low
            })
            .collect()
    }
}
