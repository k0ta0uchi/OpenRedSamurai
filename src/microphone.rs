//! Windows microphone mute control.
//!
//! The mouse's "マイクミュート" assignment is a semantic action.  It must
//! not be represented by a guessed keyboard virtual key because Windows does
//! not define a universal microphone-mute VK.  This module targets the
//! default capture endpoint through the Core Audio EndpointVolume API and
//! verifies the state again after changing it.

use std::fmt;

/// Capture endpoint role used when resolving the default microphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureRole {
    /// The endpoint selected for voice communications.
    Communications,
    /// The general default capture endpoint.
    Console,
}

impl CaptureRole {
    /// Prefer the communications microphone, then fall back to the general
    /// default capture endpoint when Windows has no communications endpoint.
    pub const fn preference_order() -> [Self; 2] {
        [Self::Communications, Self::Console]
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Communications => "communications",
            Self::Console => "console",
        }
    }
}

/// Result of a successful toggle, including the state read back from Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicrophoneMuteResult {
    pub role: CaptureRole,
    pub muted: bool,
}

/// Errors returned by the Core Audio microphone path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicrophoneMuteError {
    UnsupportedPlatform,
    ComInitialization {
        code: i32,
    },
    Enumerator {
        code: i32,
    },
    CaptureEndpointUnavailable {
        communications_code: i32,
        console_code: i32,
    },
    EndpointActivation {
        code: i32,
    },
    ReadMute {
        code: i32,
    },
    SetMute {
        code: i32,
    },
    ReadbackMismatch {
        expected: bool,
        actual: bool,
    },
}

impl fmt::Display for MicrophoneMuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                f.write_str("microphone mute is only supported on Windows")
            }
            Self::ComInitialization { code } => {
                write!(f, "Core Audio COM initialization failed (HRESULT 0x{code:08X})")
            }
            Self::Enumerator { code } => {
                write!(f, "Core Audio device enumerator creation failed (HRESULT 0x{code:08X})")
            }
            Self::CaptureEndpointUnavailable {
                communications_code,
                console_code,
            } => write!(
                f,
                "no default capture endpoint (communications HRESULT 0x{communications_code:08X}, console HRESULT 0x{console_code:08X})"
            ),
            Self::EndpointActivation { code } => write!(
                f,
                "capture endpoint volume activation failed (HRESULT 0x{code:08X})"
            ),
            Self::ReadMute { code } => {
                write!(f, "capture mute read failed (HRESULT 0x{code:08X})")
            }
            Self::SetMute { code } => {
                write!(f, "capture mute update failed (HRESULT 0x{code:08X})")
            }
            Self::ReadbackMismatch { expected, actual } => write!(
                f,
                "capture mute readback mismatch (expected {expected}, got {actual})"
            ),
        }
    }
}

impl std::error::Error for MicrophoneMuteError {}

/// Compute the state requested by one microphone-mute trigger.
pub const fn toggled_mute_state(current: bool) -> bool {
    !current
}

/// Toggle the default Windows capture endpoint and verify the new state.
pub fn toggle_default_capture_mute() -> Result<MicrophoneMuteResult, MicrophoneMuteError> {
    #[cfg(windows)]
    {
        return toggle_windows_capture_mute();
    }

    #[cfg(not(windows))]
    {
        Err(MicrophoneMuteError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
struct ComGuard {
    uninitialize: bool,
}

#[cfg(windows)]
impl ComGuard {
    fn initialize() -> Result<Self, MicrophoneMuteError> {
        use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_ok() {
            return Ok(Self { uninitialize: true });
        }
        if result == RPC_E_CHANGED_MODE {
            // The thread already belongs to a different COM apartment.  COM
            // remains usable, but this call must not uninitialize that
            // apartment when the operation ends.
            return Ok(Self {
                uninitialize: false,
            });
        }
        Err(MicrophoneMuteError::ComInitialization { code: result.0 })
    }
}

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

#[cfg(windows)]
fn toggle_windows_capture_mute() -> Result<MicrophoneMuteResult, MicrophoneMuteError> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    let _com = ComGuard::initialize()?;
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
    }
    .map_err(|error| MicrophoneMuteError::Enumerator {
        code: error.code().0,
    })?;

    let (endpoint, role) = default_capture_endpoint(&enumerator)?;
    let volume: IAudioEndpointVolume =
        unsafe { endpoint.Activate(CLSCTX_ALL, None) }.map_err(|error| {
            MicrophoneMuteError::EndpointActivation {
                code: error.code().0,
            }
        })?;
    let current = unsafe { volume.GetMute() }
        .map_err(|error| MicrophoneMuteError::ReadMute {
            code: error.code().0,
        })?
        .as_bool();
    let expected = toggled_mute_state(current);
    unsafe { volume.SetMute(expected, std::ptr::null()) }.map_err(|error| {
        MicrophoneMuteError::SetMute {
            code: error.code().0,
        }
    })?;
    let actual = unsafe { volume.GetMute() }
        .map_err(|error| MicrophoneMuteError::ReadMute {
            code: error.code().0,
        })?
        .as_bool();
    if actual != expected {
        return Err(MicrophoneMuteError::ReadbackMismatch { expected, actual });
    }

    Ok(MicrophoneMuteResult {
        role,
        muted: actual,
    })
}

#[cfg(windows)]
fn default_capture_endpoint(
    enumerator: &windows::Win32::Media::Audio::IMMDeviceEnumerator,
) -> Result<(windows::Win32::Media::Audio::IMMDevice, CaptureRole), MicrophoneMuteError> {
    use windows::Win32::Media::Audio::{eCapture, eCommunications, eConsole};

    let communications = unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications) };
    if let Ok(endpoint) = communications {
        return Ok((endpoint, CaptureRole::Communications));
    }
    let communications_code = communications
        .as_ref()
        .err()
        .map(|error| error.code().0)
        .unwrap_or_default();

    let console = unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) };
    if let Ok(endpoint) = console {
        return Ok((endpoint, CaptureRole::Console));
    }
    let console_code = console
        .as_ref()
        .err()
        .map(|error| error.code().0)
        .unwrap_or_default();

    Err(MicrophoneMuteError::CaptureEndpointUnavailable {
        communications_code,
        console_code,
    })
}

#[cfg(test)]
mod tests {
    use super::{toggled_mute_state, CaptureRole};

    #[test]
    fn toggle_flips_both_mute_states() {
        assert!(toggled_mute_state(false));
        assert!(!toggled_mute_state(true));
    }

    #[test]
    fn communications_endpoint_is_preferred_before_console_fallback() {
        assert_eq!(
            CaptureRole::preference_order(),
            [CaptureRole::Communications, CaptureRole::Console]
        );
    }
}
