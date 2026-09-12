//! Fail-closed HID transport for the RED SAMURAI configuration interface.
//!
//! The mouse is a composite HID device.  Only the vendor configuration
//! collection (`MI_02&COL02`, usage page `0xFFA0`) is accepted here; the
//! ordinary mouse and vendor-input collections are deliberately ignored.
//! This module does not infer command meanings.  Callers must provide the
//! exact, already-verified feature-report bytes they intend to transfer.
//!
//! Constructing a transport never sends a report.  Tests and dry-run code can
//! use [`DeviceTransport::mock`] (or inject a [`FeatureReportIo`]) and cannot
//! touch the HID device.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

/// RED SAMURAI's USB vendor ID.
pub const RED_SAMURAI_VENDOR_ID: u16 = 0x04D9;
/// RED SAMURAI 16400DPI mouse's USB product ID.
pub const RED_SAMURAI_PRODUCT_ID: u16 = 0xFC55;
/// Vendor-defined HID usage page for the configuration collection.
pub const CONFIGURATION_USAGE_PAGE: u16 = 0xFFA0;
/// Composite-device interface number encoded by `MI_02`.
pub const CONFIGURATION_INTERFACE_NUMBER: i32 = 2;
/// HID collection encoded by `COL02`.
pub const CONFIGURATION_COLLECTION: &str = "COL02";
/// Composite-device interface token encoded by `MI_02`.
pub const CONFIGURATION_INTERFACE_TOKEN: &str = "MI_02";
/// Length, including the report ID byte, of normal control reports.
pub const DEFAULT_CONTROL_REPORT_LENGTH: usize = 16;
/// Length, including the report ID byte, of the observed `03/F3/20` report.
/// The capture establishes the size, while the command semantics remain
/// unsupported by the apply planner.
pub const EXTENDED_CONTROL_REPORT_LENGTH: usize = 64;
/// Length, including the report ID byte, of an explicit block write.
pub const BLOCK_REPORT_LENGTH: usize = 1024;

/// Errors returned by discovery, opening, and report transfers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceError {
    /// This build does not include a HID backend for the current platform.
    UnsupportedPlatform,
    /// HID discovery failed before a device could be selected.
    Discovery { message: String },
    /// No verified RED SAMURAI configuration interface is present.
    DeviceUnavailable,
    /// The selected device did not contain the verified configuration mapping.
    UnverifiedConfigurationMapping,
    /// Opening the verified interface failed.
    Open { message: String },
    /// An already-opened interface rejected a feature transfer.
    Io {
        operation: &'static str,
        message: String,
    },
    /// The report buffer did not have the required exact length.
    InvalidReportLength { expected: usize, actual: usize },
    /// An explicit length of zero is never a valid HID report buffer.
    InvalidExpectedReportLength { expected: usize },
    /// A feature response changed the report ID supplied by the caller.
    ReportIdMismatch { expected: u8, actual: u8 },
    /// A read-only observation returned no bytes or more bytes than the exact
    /// request route can carry.  Short, non-empty responses are allowed only
    /// through the dedicated observation seam and are decoded separately.
    InvalidObservedReportLength { requested: usize, actual: usize },
    /// The caller supplied a report header that is not present in the
    /// reviewed write mapping table.  Raw buffers are rejected before they
    /// reach the HID backend so an unreviewed command cannot be sent by
    /// accident.
    UnverifiedReportHeader { report_id: u8, cmd: u8, subcmd: u8 },
    /// The supplied header is observed only as a read-only status query and
    /// must not be routed through the send path.
    ReadOnlyReport { report_id: u8, cmd: u8, subcmd: u8 },
    /// A gated write used a mapping for a different exact command header.
    VerifiedMappingMismatch {
        expected_report_id: u8,
        expected_cmd: u8,
        expected_subcmd: u8,
        actual_report_id: u8,
        actual_cmd: u8,
        actual_subcmd: u8,
    },
    /// The requested report ID is not one of the observed feature-report
    /// IDs for this device.
    UnverifiedReportId { report_id: u8 },
    /// The mock backend was asked for a response that was not queued.
    MockReportUnavailable,
    /// A mock-only operation was attempted on a hardware transport.
    MockOnly { operation: &'static str },
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("RED SAMURAI HID transport is Windows-only"),
            Self::Discovery { message } => write!(f, "HID discovery failed: {message}"),
            Self::DeviceUnavailable => {
                f.write_str("RED SAMURAI configuration interface is not available")
            }
            Self::UnverifiedConfigurationMapping => {
                f.write_str("RED SAMURAI configuration interface mapping is unverified")
            }
            Self::Open { message } => {
                write!(f, "opening RED SAMURAI HID interface failed: {message}")
            }
            Self::Io { operation, message } => {
                write!(f, "HID {operation} failed: {message}")
            }
            Self::InvalidReportLength { expected, actual } => {
                write!(
                    f,
                    "invalid HID report length: expected {expected} bytes, got {actual}"
                )
            }
            Self::InvalidExpectedReportLength { expected } => {
                write!(f, "invalid expected HID report length: {expected}")
            }
            Self::ReportIdMismatch { expected, actual } => {
                write!(
                    f,
                    "HID report ID mismatch: expected 0x{expected:02X}, got 0x{actual:02X}"
                )
            }
            Self::InvalidObservedReportLength { requested, actual } => write!(
                f,
                "invalid observed HID report length: request route {requested} bytes, response {actual} bytes"
            ),
            Self::UnverifiedReportHeader {
                report_id,
                cmd,
                subcmd,
            } => write!(
                f,
                "unverified HID report header: {report_id:02X}/{cmd:02X}/{subcmd:02X}"
            ),
            Self::UnverifiedReportId { report_id } => {
                write!(f, "unverified HID report ID: 0x{report_id:02X}")
            }
            Self::ReadOnlyReport {
                report_id,
                cmd,
                subcmd,
            } => write!(
                f,
                "read-only HID report cannot be sent: {report_id:02X}/{cmd:02X}/{subcmd:02X}"
            ),
            Self::VerifiedMappingMismatch {
                expected_report_id,
                expected_cmd,
                expected_subcmd,
                actual_report_id,
                actual_cmd,
                actual_subcmd,
            } => write!(
                f,
                "verified HID mapping mismatch: expected {expected_report_id:02X}/{expected_cmd:02X}/{expected_subcmd:02X}, got {actual_report_id:02X}/{actual_cmd:02X}/{actual_subcmd:02X}"
            ),
            Self::MockReportUnavailable => f.write_str("mock HID report queue is empty"),
            Self::MockOnly { operation } => {
                write!(f, "{operation} is only available on a mock transport")
            }
        }
    }
}

impl Error for DeviceError {}

/// A HID interface candidate with the fields needed for fail-closed filtering.
///
/// Values returned by [`discover_configuration_devices`] are already verified
/// against the VID, PID, usage page, interface number, and `COL02` path token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredDevice {
    path: String,
    vendor_id: u16,
    product_id: u16,
    usage_page: u16,
    interface_number: i32,
}

impl DiscoveredDevice {
    /// Construct a candidate for filtering or tests.
    pub fn new(
        path: impl Into<String>,
        vendor_id: u16,
        product_id: u16,
        usage_page: u16,
        interface_number: i32,
    ) -> Self {
        Self {
            path: path.into(),
            vendor_id,
            product_id,
            usage_page,
            interface_number,
        }
    }

    /// Windows HID device path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// USB vendor ID.
    pub const fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    /// USB product ID.
    pub const fn product_id(&self) -> u16 {
        self.product_id
    }

    /// HID usage page.
    pub const fn usage_page(&self) -> u16 {
        self.usage_page
    }

    /// Composite USB interface number (`MI_02` is 2).
    pub const fn interface_number(&self) -> i32 {
        self.interface_number
    }
}

fn has_collection_token(path: &str) -> bool {
    path.to_ascii_uppercase().split('&').any(|segment| {
        segment
            .split('#')
            .next()
            .is_some_and(|token| token == CONFIGURATION_COLLECTION)
    })
}

fn has_interface_token(path: &str) -> bool {
    path.to_ascii_uppercase().split('&').any(|segment| {
        segment
            .split('#')
            .next()
            .is_some_and(|token| token == CONFIGURATION_INTERFACE_TOKEN)
    })
}

/// Return whether a candidate is the verified configuration collection.
///
/// Every field is required.  In particular, VID/PID-only matching is not
/// sufficient for this composite HID device.
pub fn is_configuration_interface(candidate: &DiscoveredDevice) -> bool {
    candidate.vendor_id == RED_SAMURAI_VENDOR_ID
        && candidate.product_id == RED_SAMURAI_PRODUCT_ID
        && candidate.usage_page == CONFIGURATION_USAGE_PAGE
        && candidate.interface_number == CONFIGURATION_INTERFACE_NUMBER
        && has_interface_token(&candidate.path)
        && has_collection_token(&candidate.path)
}

/// Filter an iterator to the verified configuration collection(s).
pub fn select_configuration_interfaces<'a>(
    devices: impl IntoIterator<Item = &'a DiscoveredDevice>,
) -> Vec<&'a DiscoveredDevice> {
    devices
        .into_iter()
        .filter(|candidate| is_configuration_interface(candidate))
        .collect()
}

/// Enumerate attached, verified RED SAMURAI configuration interfaces.
///
/// Discovery only enumerates and inspects HID metadata; it never sends a
/// report.  A missing device is represented by an empty `Ok` vector so callers
/// can distinguish absence from an OS/API discovery failure.
#[cfg(windows)]
pub fn discover_configuration_devices() -> Result<Vec<DiscoveredDevice>, DeviceError> {
    let api = hidapi::HidApi::new().map_err(|error| DeviceError::Discovery {
        message: error.to_string(),
    })?;

    let mut devices = Vec::new();
    for info in api.device_list() {
        let candidate = DiscoveredDevice::new(
            info.path().to_string_lossy(),
            info.vendor_id(),
            info.product_id(),
            info.usage_page(),
            info.interface_number(),
        );
        if is_configuration_interface(&candidate) {
            devices.push(candidate);
        }
    }
    Ok(devices)
}

/// The transport intentionally has no non-Windows fallback that could select
/// an unverified HID backend.
#[cfg(not(windows))]
pub fn discover_configuration_devices() -> Result<Vec<DiscoveredDevice>, DeviceError> {
    Err(DeviceError::UnsupportedPlatform)
}

/// Check whether at least one verified configuration interface is attached.
pub fn is_available() -> Result<bool, DeviceError> {
    Ok(!discover_configuration_devices()?.is_empty())
}

/// Alias for callers that prefer a noun-like availability API.
pub fn device_available() -> Result<bool, DeviceError> {
    is_available()
}

/// A backend used by [`DeviceTransport`].  Implement this trait to inject a
/// deterministic dry-run backend in tests; methods receive and return complete
/// buffers including the report ID byte.
pub trait FeatureReportIo: std::any::Any {
    /// Send one already-sized feature report.
    fn send_feature_report_raw(&mut self, report: &[u8]) -> Result<(), DeviceError>;

    /// Fill `buffer` with one feature report and return its exact byte count.
    fn get_feature_report_raw(&mut self, buffer: &mut [u8]) -> Result<usize, DeviceError>;

    /// Fill `buffer` with one read-only observation and return its exact byte
    /// count.  HID GET_REPORT responses can be shorter than their requested
    /// 16/64/1024-byte route; the dedicated transport seam handles that
    /// bounded response shape without weakening strict report reads.
    fn get_feature_report_observation_raw(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<usize, DeviceError> {
        self.get_feature_report_raw(buffer)
    }
}

/// An opened hardware or mock configuration transport.
pub struct DeviceTransport {
    backend: Box<dyn FeatureReportIo>,
}

impl fmt::Debug for DeviceTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceTransport").finish_non_exhaustive()
    }
}

impl DeviceTransport {
    /// Wrap an injected backend.  This is the dependency-injection seam for
    /// tests and dry-run operation; it performs no hardware I/O itself.
    pub fn from_backend<B>(backend: B) -> Self
    where
        B: FeatureReportIo + 'static,
    {
        Self {
            backend: Box::new(backend),
        }
    }

    /// Construct a transport that records writes and serves queued responses.
    pub fn mock() -> Self {
        Self::from_backend(MockFeatureBackend::default())
    }

    /// Alias emphasizing that this transport is safe for startup previews.
    pub fn dry_run() -> Self {
        Self::mock()
    }

    /// Queue a complete feature response for a mock transport.
    pub fn queue_mock_feature_report(
        &mut self,
        report: impl Into<Vec<u8>>,
    ) -> Result<(), DeviceError> {
        let Some(mock) = self
            .backend
            .as_any_mut()
            .downcast_mut::<MockFeatureBackend>()
        else {
            return Err(DeviceError::MockOnly {
                operation: "queue_mock_feature_report",
            });
        };
        mock.queued_reports.push_back(report.into());
        Ok(())
    }

    /// Return reports recorded by a mock transport, or `None` for hardware.
    pub fn sent_mock_reports(&self) -> Option<&[Vec<u8>]> {
        self.backend
            .as_any()
            .downcast_ref::<MockFeatureBackend>()
            .map(|mock| mock.sent_reports.as_slice())
    }

    /// Send a normal control feature report, exactly 16 bytes including ID.
    pub fn send_feature_report(&mut self, report: &[u8]) -> Result<(), DeviceError> {
        self.send_feature_report_exact(report, DEFAULT_CONTROL_REPORT_LENGTH)
    }

    /// Send an observed feature report with a caller-supplied exact length.
    ///
    /// The explicit length is required for 1024-byte block writes.  Passing a
    /// 64- or 1024-byte buffer to [`Self::send_feature_report`] is rejected as
    /// a control-report length error.  The first three bytes must also match a
    /// reviewed write mapping; observed-but-unverified rows and raw buffers
    /// with an unknown header are rejected before backend I/O.
    pub fn send_feature_report_exact(
        &mut self,
        report: &[u8],
        expected_len: usize,
    ) -> Result<(), DeviceError> {
        validate_report_length(report.len(), expected_len)?;
        validate_send_header(report, expected_len)?;
        self.backend.send_feature_report_raw(report)
    }

    /// Send a report through an explicit, reviewed command-mapping gate.
    ///
    /// A [`VerifiedCommandMapping`] cannot be fabricated by downstream
    /// callers; the protocol module exposes only mappings that a reviewer has
    /// added with concrete capture evidence.  This API remains available now
    /// so a future verified mapping can be adopted without reopening the raw
    /// byte path.
    fn send_verified_feature_report(
        &mut self,
        mapping: &crate::device_protocol::VerifiedCommandMapping,
        report: &[u8],
    ) -> Result<(), DeviceError> {
        validate_report_length(report.len(), mapping.frame_len())?;
        validate_verified_send_header(report, mapping)?;
        self.backend.send_feature_report_raw(report)
    }

    /// Send a validated, mapping-bound write token.
    pub fn send_verified_write(
        &mut self,
        write: &crate::device_protocol::VerifiedWrite,
    ) -> Result<(), DeviceError> {
        let report = write.as_bytes().map_err(|error| DeviceError::Io {
            operation: "encode verified feature report",
            message: error.to_string(),
        })?;
        self.send_verified_feature_report(write.mapping(), &report)
    }

    /// Send one complete, evidence-backed Apply sequence.
    ///
    /// A sequence token is validated again at this final device boundary so
    /// that only the reviewed report count, order, headers, lengths, and
    /// PollingRate value can reach the backend.  The individual reports are
    /// submitted in token order and the returned count is the number of
    /// backend calls that completed successfully.  This method is separate
    /// from the legacy raw-report APIs: a standalone `ReportFrame` can never
    /// be promoted into this path.
    pub fn send_verified_apply_sequence(
        &mut self,
        sequence: &crate::device_protocol::VerifiedApplySequence,
    ) -> Result<usize, DeviceError> {
        let validated =
            crate::device_protocol::VerifiedApplySequence::try_from_frames(sequence.frames())
                .map_err(|error| DeviceError::Io {
                    operation: "validate verified Apply sequence",
                    message: error.to_string(),
                })?;

        let mut sent = 0;
        for frame in validated.frames() {
            let report = frame.encode().map_err(|error| DeviceError::Io {
                operation: "encode verified Apply report",
                message: error.to_string(),
            })?;
            self.backend.send_feature_report_raw(&report)?;
            sent += 1;
        }
        Ok(sent)
    }

    /// Convenience method whose explicit implementation makes a block-write
    /// intent visible at every call site.
    pub fn send_block_feature_report(&mut self, report: &[u8]) -> Result<(), DeviceError> {
        self.send_feature_report_exact(report, BLOCK_REPORT_LENGTH)
    }

    /// Send the observed 64-byte extended report only when a caller has an
    /// explicit wire-level reason to do so.  This method does not grant any
    /// command semantic or profile-mapping authority.
    pub fn send_extended_feature_report(&mut self, report: &[u8]) -> Result<(), DeviceError> {
        self.send_feature_report_exact(report, EXTENDED_CONTROL_REPORT_LENGTH)
    }

    /// Read a normal 16-byte control feature report.
    pub fn get_feature_report(&mut self, report_id: u8) -> Result<Vec<u8>, DeviceError> {
        self.get_feature_report_exact(report_id, DEFAULT_CONTROL_REPORT_LENGTH)
    }

    /// Read a feature report with a caller-supplied exact length.
    pub fn get_feature_report_exact(
        &mut self,
        report_id: u8,
        expected_len: usize,
    ) -> Result<Vec<u8>, DeviceError> {
        validate_report_length(expected_len, expected_len)?;
        validate_get_report_shape(report_id, expected_len)?;
        let mut report = vec![0; expected_len];
        self.get_feature_report_into(report_id, &mut report, expected_len)?;
        Ok(report)
    }

    /// Read into a caller-owned buffer and validate both requested and actual
    /// lengths.  The report ID and length must be one of the observed device
    /// shapes.  The buffer includes the report ID byte.
    pub fn get_feature_report_into(
        &mut self,
        report_id: u8,
        buffer: &mut [u8],
        expected_len: usize,
    ) -> Result<usize, DeviceError> {
        validate_report_length(buffer.len(), expected_len)?;
        validate_get_report_shape(report_id, expected_len)?;
        buffer[0] = report_id;
        let actual_len = self.backend.get_feature_report_raw(buffer)?;
        validate_report_length(actual_len, expected_len)?;
        if buffer[0] != report_id {
            return Err(DeviceError::ReportIdMismatch {
                expected: report_id,
                actual: buffer[0],
            });
        }
        Ok(actual_len)
    }

    /// Read one exact, observed GET_REPORT route for a device-to-host
    /// observation.
    ///
    /// The request route length is still strict (16, 64, or 1024 bytes for an
    /// observed report ID), but the device may return a shorter non-empty
    /// response.  This method never sends a feature report and does not accept
    /// a command header, so it cannot become a standalone speculative write.
    pub fn get_feature_report_observation(
        &mut self,
        report_id: u8,
        request_len: usize,
    ) -> Result<Vec<u8>, DeviceError> {
        validate_report_length(request_len, request_len)?;
        validate_get_report_shape(report_id, request_len)?;
        let mut report = vec![0; request_len];
        report[0] = report_id;
        let actual_len = self
            .backend
            .get_feature_report_observation_raw(&mut report)?;
        if actual_len == 0 || actual_len > request_len {
            return Err(DeviceError::InvalidObservedReportLength {
                requested: request_len,
                actual: actual_len,
            });
        }
        if report[0] != report_id {
            return Err(DeviceError::ReportIdMismatch {
                expected: report_id,
                actual: report[0],
            });
        }
        report.truncate(actual_len);
        Ok(report)
    }

    /// Observe the exact PollingRate response shape recorded by the official
    /// post-Apply readback.  The request is a read-only report-ID route; no
    /// command mapping or standalone write frame is constructed.
    pub fn observe_polling_rate_readback(
        &mut self,
    ) -> Result<crate::device_protocol::PollingRateReadback, DeviceError> {
        let report = self.get_feature_report_observation(
            crate::device_protocol::REPORT_ID_CONFIG,
            crate::device_protocol::REPORT_LEN,
        )?;
        crate::device_protocol::PollingRateReadback::decode(&report).map_err(|error| {
            DeviceError::Io {
                operation: "decode polling-rate readback",
                message: error.to_string(),
            }
        })
    }

    /// Observe the selected-DPI response recorded by the official
    /// Apply/reconnect/readback A/B capture. The request uses report 03's
    /// 64-byte GET_REPORT route; after the required Apply/reconnect context the
    /// device returns a validated 40-byte response. A standalone GET may
    /// return a short alternate response and is rejected by the decoder. This
    /// method never sends a feature report.
    pub fn observe_dpi_selection_readback(
        &mut self,
    ) -> Result<crate::device_protocol::DpiSelectionReadback, DeviceError> {
        let report = self.get_feature_report_observation(
            crate::device_protocol::DPI_SELECTION_READBACK_REPORT_ID,
            crate::device_protocol::DPI_SELECTION_READBACK_REQUEST_LEN,
        )?;
        crate::device_protocol::DpiSelectionReadback::decode(&report).map_err(|error| {
            let message = match &error {
                crate::device_protocol::DpiSelectionReadbackError::InvalidLength {
                    actual: 8,
                    ..
                } => format!(
                    "{error}; standalone Report-03 GET returned short/alternate response bytes={}; selected-DPI readback requires the full Apply/reconnect readback sequence",
                    format_report_bytes(&report)
                ),
                _ => error.to_string(),
            };
            DeviceError::Io {
                operation: "decode DPI selection readback",
                message,
            }
        })
    }
}

fn format_report_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_report_length(actual: usize, expected: usize) -> Result<(), DeviceError> {
    if expected == 0 || expected > BLOCK_REPORT_LENGTH {
        return Err(DeviceError::InvalidExpectedReportLength { expected });
    }
    if actual != expected {
        return Err(DeviceError::InvalidReportLength { expected, actual });
    }
    Ok(())
}

fn validate_send_header(report: &[u8], expected_len: usize) -> Result<(), DeviceError> {
    if report.len() < crate::device_protocol::FRAME_HEADER_LEN {
        return Err(DeviceError::InvalidReportLength {
            expected: crate::device_protocol::REPORT_LEN,
            actual: report.len(),
        });
    }
    let report_id = report[crate::device_protocol::REPORT_ID_OFFSET];
    let cmd = report[crate::device_protocol::CMD_OFFSET];
    let subcmd = report[crate::device_protocol::SUBCMD_OFFSET];
    let Some(info) = crate::device_protocol::command_info(report_id, cmd, subcmd) else {
        return Err(DeviceError::UnverifiedReportHeader {
            report_id,
            cmd,
            subcmd,
        });
    };
    if matches!(
        info.capability,
        crate::device_protocol::CommandCapability::ReadOnly
    ) {
        return Err(DeviceError::ReadOnlyReport {
            report_id,
            cmd,
            subcmd,
        });
    }
    if info.frame_len != expected_len {
        return Err(DeviceError::InvalidReportLength {
            expected: info.frame_len,
            actual: report.len(),
        });
    }
    // An observed row is not a write authorization.  Even if a future
    // capture adds a reviewed mapping, callers must use the explicit mapping
    // API below rather than re-opening this raw byte path.
    Err(DeviceError::UnverifiedReportHeader {
        report_id,
        cmd,
        subcmd,
    })
}

fn validate_verified_send_header(
    report: &[u8],
    mapping: &crate::device_protocol::VerifiedCommandMapping,
) -> Result<(), DeviceError> {
    if report.len() < crate::device_protocol::FRAME_HEADER_LEN {
        return Err(DeviceError::InvalidReportLength {
            expected: mapping.frame_len(),
            actual: report.len(),
        });
    }

    let report_id = report[crate::device_protocol::REPORT_ID_OFFSET];
    let cmd = report[crate::device_protocol::CMD_OFFSET];
    let subcmd = report[crate::device_protocol::SUBCMD_OFFSET];
    if (report_id, cmd, subcmd) != (mapping.report_id(), mapping.cmd(), mapping.subcmd()) {
        return Err(DeviceError::VerifiedMappingMismatch {
            expected_report_id: mapping.report_id(),
            expected_cmd: mapping.cmd(),
            expected_subcmd: mapping.subcmd(),
            actual_report_id: report_id,
            actual_cmd: cmd,
            actual_subcmd: subcmd,
        });
    }

    let Some(info) = crate::device_protocol::command_info(report_id, cmd, subcmd) else {
        return Err(DeviceError::UnverifiedReportHeader {
            report_id,
            cmd,
            subcmd,
        });
    };
    if matches!(
        info.capability,
        crate::device_protocol::CommandCapability::ReadOnly
    ) {
        return Err(DeviceError::ReadOnlyReport {
            report_id,
            cmd,
            subcmd,
        });
    }
    if info.frame_len != mapping.frame_len() {
        return Err(DeviceError::InvalidReportLength {
            expected: info.frame_len,
            actual: report.len(),
        });
    }
    if !mapping.is_registered() {
        return Err(DeviceError::UnverifiedReportHeader {
            report_id,
            cmd,
            subcmd,
        });
    }
    Ok(())
}

fn validate_get_report_shape(report_id: u8, expected_len: usize) -> Result<(), DeviceError> {
    let valid = match report_id {
        crate::device_protocol::REPORT_ID_CONFIG => {
            expected_len == crate::device_protocol::REPORT_LEN
        }
        crate::device_protocol::REPORT_ID_CONFIG_EXTENDED => {
            expected_len == crate::device_protocol::REPORT_LEN
                || expected_len == crate::device_protocol::EXTENDED_REPORT_LEN
        }
        crate::device_protocol::REPORT_ID_BULK => {
            expected_len == crate::device_protocol::BULK_REPORT_LEN
        }
        _ => false,
    };
    if !valid {
        if !matches!(
            report_id,
            crate::device_protocol::REPORT_ID_CONFIG
                | crate::device_protocol::REPORT_ID_CONFIG_EXTENDED
                | crate::device_protocol::REPORT_ID_BULK
        ) {
            return Err(DeviceError::UnverifiedReportId { report_id });
        }
        return Err(DeviceError::InvalidReportLength {
            expected: match report_id {
                crate::device_protocol::REPORT_ID_CONFIG => crate::device_protocol::REPORT_LEN,
                crate::device_protocol::REPORT_ID_BULK => crate::device_protocol::BULK_REPORT_LEN,
                _ => crate::device_protocol::REPORT_LEN,
            },
            actual: expected_len,
        });
    }
    Ok(())
}

/// Open the first verified RED SAMURAI configuration interface.
#[cfg(windows)]
pub fn open_configuration_device() -> Result<DeviceTransport, DeviceError> {
    let api = hidapi::HidApi::new().map_err(|error| DeviceError::Discovery {
        message: error.to_string(),
    })?;
    let info = api.device_list().find(|info| {
        let candidate = DiscoveredDevice::new(
            info.path().to_string_lossy(),
            info.vendor_id(),
            info.product_id(),
            info.usage_page(),
            info.interface_number(),
        );
        is_configuration_interface(&candidate)
    });
    let info = info.ok_or(DeviceError::DeviceUnavailable)?;
    let device = info.open_device(&api).map_err(|error| DeviceError::Open {
        message: error.to_string(),
    })?;
    Ok(DeviceTransport::from_backend(HidFeatureBackend { device }))
}

/// No VID/PID-only or guessed-interface fallback is permitted off Windows.
#[cfg(not(windows))]
pub fn open_configuration_device() -> Result<DeviceTransport, DeviceError> {
    Err(DeviceError::UnsupportedPlatform)
}

/// Short alias for opening the verified configuration interface.
pub fn open_device() -> Result<DeviceTransport, DeviceError> {
    open_configuration_device()
}

impl dyn FeatureReportIo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Default)]
struct MockFeatureBackend {
    sent_reports: Vec<Vec<u8>>,
    queued_reports: VecDeque<Vec<u8>>,
}

impl FeatureReportIo for MockFeatureBackend {
    fn send_feature_report_raw(&mut self, report: &[u8]) -> Result<(), DeviceError> {
        self.sent_reports.push(report.to_vec());
        Ok(())
    }

    fn get_feature_report_raw(&mut self, buffer: &mut [u8]) -> Result<usize, DeviceError> {
        let report = self
            .queued_reports
            .pop_front()
            .ok_or(DeviceError::MockReportUnavailable)?;
        validate_report_length(report.len(), buffer.len())?;
        buffer.copy_from_slice(&report);
        Ok(report.len())
    }

    fn get_feature_report_observation_raw(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<usize, DeviceError> {
        let report = self
            .queued_reports
            .pop_front()
            .ok_or(DeviceError::MockReportUnavailable)?;
        if report.is_empty() || report.len() > buffer.len() {
            return Err(DeviceError::InvalidObservedReportLength {
                requested: buffer.len(),
                actual: report.len(),
            });
        }
        buffer[..report.len()].copy_from_slice(&report);
        Ok(report.len())
    }
}

#[cfg(windows)]
struct HidFeatureBackend {
    device: hidapi::HidDevice,
}

#[cfg(windows)]
impl FeatureReportIo for HidFeatureBackend {
    fn send_feature_report_raw(&mut self, report: &[u8]) -> Result<(), DeviceError> {
        self.device
            .send_feature_report(report)
            .map_err(|error| DeviceError::Io {
                operation: "send feature report",
                message: error.to_string(),
            })
    }

    fn get_feature_report_raw(&mut self, buffer: &mut [u8]) -> Result<usize, DeviceError> {
        self.device
            .get_feature_report(buffer)
            .map_err(|error| DeviceError::Io {
                operation: "get feature report",
                message: error.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        path: &str,
        vendor_id: u16,
        product_id: u16,
        usage_page: u16,
        interface: i32,
    ) -> DiscoveredDevice {
        DiscoveredDevice::new(path, vendor_id, product_id, usage_page, interface)
    }

    #[test]
    fn filtering_requires_the_verified_composite_configuration_interface() {
        let candidates = vec![
            candidate(
                r#"\\?\HID#VID_04D9&PID_FC55&MI_02&COL02#{guid}"#,
                RED_SAMURAI_VENDOR_ID,
                RED_SAMURAI_PRODUCT_ID,
                CONFIGURATION_USAGE_PAGE,
                CONFIGURATION_INTERFACE_NUMBER,
            ),
            candidate(
                r#"\\?\HID#VID_04D9&PID_FC55&MI_02&COL01#{guid}"#,
                RED_SAMURAI_VENDOR_ID,
                RED_SAMURAI_PRODUCT_ID,
                CONFIGURATION_USAGE_PAGE,
                CONFIGURATION_INTERFACE_NUMBER,
            ),
            candidate(
                r#"\\?\HID#VID_04D9&PID_FC55&MI_01&COL02#{guid}"#,
                RED_SAMURAI_VENDOR_ID,
                RED_SAMURAI_PRODUCT_ID,
                CONFIGURATION_USAGE_PAGE,
                1,
            ),
            candidate(
                r#"\\?\HID#VID_04D9&PID_FC55&MI_01&COL02#{guid}"#,
                RED_SAMURAI_VENDOR_ID,
                RED_SAMURAI_PRODUCT_ID,
                CONFIGURATION_USAGE_PAGE,
                CONFIGURATION_INTERFACE_NUMBER,
            ),
            candidate(
                r#"\\?\HID#VID_04D9&PID_FC55&COL02#{guid}"#,
                RED_SAMURAI_VENDOR_ID,
                RED_SAMURAI_PRODUCT_ID,
                CONFIGURATION_USAGE_PAGE,
                CONFIGURATION_INTERFACE_NUMBER,
            ),
            candidate(
                r#"\\?\HID#VID_04D9&PID_FC55&MI_02&COL02#{guid}"#,
                RED_SAMURAI_VENDOR_ID,
                RED_SAMURAI_PRODUCT_ID,
                0x0002,
                CONFIGURATION_INTERFACE_NUMBER,
            ),
            candidate(
                r#"\\?\HID#VID_1234&PID_FC55&MI_02&COL02#{guid}"#,
                0x1234,
                RED_SAMURAI_PRODUCT_ID,
                CONFIGURATION_USAGE_PAGE,
                CONFIGURATION_INTERFACE_NUMBER,
            ),
        ];

        let selected = select_configuration_interfaces(&candidates);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].usage_page(), CONFIGURATION_USAGE_PAGE);
        assert!(is_configuration_interface(selected[0]));
    }

    #[test]
    fn raw_observed_command_send_is_rejected_without_a_verified_mapping() {
        let mut transport = DeviceTransport::mock();
        let mut report = vec![0; DEFAULT_CONTROL_REPORT_LENGTH];
        report[..3].copy_from_slice(&[0x02, 0xF1, 0x01]);
        let error = transport
            .send_feature_report(&report)
            .expect_err("observed command is not write-authorized");
        assert_eq!(
            error,
            DeviceError::UnverifiedReportHeader {
                report_id: 0x02,
                cmd: 0xF1,
                subcmd: 0x01,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn reviewed_token_is_rejected_when_its_mapping_is_not_in_the_registry() {
        let mut transport = DeviceTransport::mock();
        let mapping = crate::device_protocol::test_verified_command_mapping(
            0x02,
            0xF1,
            0x01,
            DEFAULT_CONTROL_REPORT_LENGTH,
        );
        let mut report = [0u8; DEFAULT_CONTROL_REPORT_LENGTH];
        report[..3].copy_from_slice(&[0x02, 0xF1, 0x01]);

        let error = transport
            .send_verified_feature_report(&mapping, &report)
            .expect_err("a reviewed fixture is not write-authorized until registered");
        assert_eq!(
            error,
            DeviceError::UnverifiedReportHeader {
                report_id: 0x02,
                cmd: 0xF1,
                subcmd: 0x01,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn block_send_requires_an_explicit_length() {
        let mut transport = DeviceTransport::mock();
        let mut block = vec![0; BLOCK_REPORT_LENGTH];
        block[..3].copy_from_slice(&[0x04, 0xF3, 0xC8]);

        let error = transport
            .send_feature_report(&block)
            .expect_err("a block must not use the control default");
        assert_eq!(
            error,
            DeviceError::InvalidReportLength {
                expected: DEFAULT_CONTROL_REPORT_LENGTH,
                actual: BLOCK_REPORT_LENGTH,
            }
        );

        let error = transport
            .send_feature_report_exact(&block, BLOCK_REPORT_LENGTH)
            .expect_err("an observed block is not write-authorized");
        assert_eq!(
            error,
            DeviceError::UnverifiedReportHeader {
                report_id: 0x04,
                cmd: 0xF3,
                subcmd: 0xC8,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());

        let mapping = crate::device_protocol::test_verified_command_mapping(
            0x04,
            0xF3,
            0xC8,
            BLOCK_REPORT_LENGTH,
        );
        let error = transport
            .send_verified_feature_report(&mapping, &block)
            .expect_err("a test-only mapping is not registered write authority");
        assert_eq!(
            error,
            DeviceError::UnverifiedReportHeader {
                report_id: 0x04,
                cmd: 0xF3,
                subcmd: 0xC8,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn extended_report_requires_an_explicit_length() {
        let mut transport = DeviceTransport::mock();
        let mut report = vec![0; EXTENDED_CONTROL_REPORT_LENGTH];
        report[..3].copy_from_slice(&[0x03, 0xF3, 0x20]);

        let error = transport
            .send_feature_report(&report)
            .expect_err("extended reports must not use the 16-byte default");
        assert_eq!(
            error,
            DeviceError::InvalidReportLength {
                expected: DEFAULT_CONTROL_REPORT_LENGTH,
                actual: EXTENDED_CONTROL_REPORT_LENGTH,
            }
        );

        let error = transport
            .send_extended_feature_report(&report)
            .expect_err("observed extended command is not write-authorized");
        assert_eq!(
            error,
            DeviceError::UnverifiedReportHeader {
                report_id: 0x03,
                cmd: 0xF3,
                subcmd: 0x20,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());

        let mapping = crate::device_protocol::test_verified_command_mapping(
            0x03,
            0xF3,
            0x20,
            EXTENDED_CONTROL_REPORT_LENGTH,
        );
        let error = transport
            .send_verified_feature_report(&mapping, &report)
            .expect_err("a test-only mapping is not registered write authority");
        assert_eq!(
            error,
            DeviceError::UnverifiedReportHeader {
                report_id: 0x03,
                cmd: 0xF3,
                subcmd: 0x20,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn get_validates_response_length_and_report_id() {
        let mut transport = DeviceTransport::mock();
        let mut response = vec![0; DEFAULT_CONTROL_REPORT_LENGTH];
        response[0] = 0x02;
        transport
            .queue_mock_feature_report(response.clone())
            .unwrap();

        let actual = transport.get_feature_report(0x02).unwrap();
        assert_eq!(actual, response);
    }

    #[test]
    fn get_rejects_a_short_response_without_panicking() {
        let mut transport = DeviceTransport::mock();
        transport.queue_mock_feature_report(vec![0x02; 3]).unwrap();

        let error = transport
            .get_feature_report(0x02)
            .expect_err("short response must fail closed");
        assert_eq!(
            error,
            DeviceError::InvalidReportLength {
                expected: DEFAULT_CONTROL_REPORT_LENGTH,
                actual: 3,
            }
        );
    }

    #[test]
    fn zero_expected_length_is_rejected_before_buffer_access() {
        let mut transport = DeviceTransport::mock();
        let error = transport
            .send_feature_report_exact(&[], 0)
            .expect_err("zero-length reports are invalid");
        assert_eq!(
            error,
            DeviceError::InvalidExpectedReportLength { expected: 0 }
        );
    }

    #[test]
    fn raw_send_rejects_an_unverified_header_before_backend_io() {
        let mut transport = DeviceTransport::mock();
        let report = vec![0x02; DEFAULT_CONTROL_REPORT_LENGTH];
        let error = transport
            .send_feature_report(&report)
            .expect_err("unknown command must fail closed");
        assert_eq!(
            error,
            DeviceError::UnverifiedReportHeader {
                report_id: 0x02,
                cmd: 0x02,
                subcmd: 0x02,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn raw_send_rejects_the_read_only_status_header_before_backend_io() {
        let mut transport = DeviceTransport::mock();
        let mut report = vec![0; DEFAULT_CONTROL_REPORT_LENGTH];
        report[..3].copy_from_slice(&[0x02, 0xF2, 0x2C]);
        let error = transport
            .send_feature_report(&report)
            .expect_err("status queries must use the read path");
        assert_eq!(
            error,
            DeviceError::ReadOnlyReport {
                report_id: 0x02,
                cmd: 0xF2,
                subcmd: 0x2C,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn verified_mapping_mismatch_is_rejected_before_backend_io() {
        let mut transport = DeviceTransport::mock();
        let mapping = crate::device_protocol::test_verified_command_mapping(
            0x02,
            0xF1,
            0x01,
            DEFAULT_CONTROL_REPORT_LENGTH,
        );
        let mut report = vec![0; DEFAULT_CONTROL_REPORT_LENGTH];
        report[..3].copy_from_slice(&[0x02, 0xF1, 0x02]);

        let error = transport
            .send_verified_feature_report(&mapping, &report)
            .expect_err("the mapping must match the complete report header");
        assert!(matches!(error, DeviceError::VerifiedMappingMismatch { .. }));
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn explicit_length_must_match_the_observed_header() {
        let mut transport = DeviceTransport::mock();
        let report = vec![0; EXTENDED_CONTROL_REPORT_LENGTH];
        let mut report = report;
        report[..3].copy_from_slice(&[0x02, 0xF1, 0x01]);
        let error = transport
            .send_feature_report_exact(&report, EXTENDED_CONTROL_REPORT_LENGTH)
            .expect_err("standard header cannot use the 64-byte length");
        assert_eq!(
            error,
            DeviceError::InvalidReportLength {
                expected: DEFAULT_CONTROL_REPORT_LENGTH,
                actual: EXTENDED_CONTROL_REPORT_LENGTH,
            }
        );
        assert!(transport.sent_mock_reports().unwrap().is_empty());
    }

    #[test]
    fn get_rejects_an_unverified_report_id_before_backend_io() {
        let mut transport = DeviceTransport::mock();
        let error = transport
            .get_feature_report(0x7F)
            .expect_err("unknown report IDs must fail closed");
        assert_eq!(error, DeviceError::UnverifiedReportId { report_id: 0x7F });
    }

    #[test]
    fn verified_apply_sequence_replays_the_exact_ordered_reports() {
        let sequence = crate::device_protocol::VerifiedApplySequence::for_125_hz()
            .expect("the audited 125 Hz sequence must be constructible");
        let mut transport = DeviceTransport::mock();

        let sent = transport
            .send_verified_apply_sequence(&sequence)
            .expect("the mock backend should accept every validated report");

        assert_eq!(sent, crate::device_protocol::APPLY_SEQUENCE_FRAME_COUNT);
        let reports = transport.sent_mock_reports().unwrap();
        assert_eq!(reports.len(), sent);
        assert_eq!(reports[0], sequence.frames()[0].encode().unwrap());
        assert_eq!(
            reports[crate::device_protocol::APPLY_POLLING_RATE_FRAME_INDEX]
                [crate::device_protocol::POLLING_RATE_OBSERVED_OFFSET],
            crate::device_protocol::APPLY_POLLING_RATE_WIRE_VALUE_125_HZ
        );
        assert_eq!(
            reports[15].len(),
            crate::device_protocol::EXTENDED_REPORT_LEN
        );
    }
}
