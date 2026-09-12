//! Current-user installer and GitHub release updater.
//!
//! The release package contains `OpenRedSamurai-Setup.exe` beside the main
//! application.  This module keeps the installer boundary in Rust so a fresh
//! install or an update does not depend on a batch file or an execution-policy
//! decision in PowerShell.  The old PowerShell scripts remain available for
//! audit and backwards-compatible automation.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

pub const PRODUCT_NAME: &str = "RED SAMURAI 16400DPI Gaming Mouse";
pub const PRODUCT_EXECUTABLE_NAME: &str = "redsamurai-config.exe";
pub const INSTALLER_EXECUTABLE_NAME: &str = "OpenRedSamurai-Setup.exe";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPOSITORY: &str = "k0ta0uchi/OpenRedSamurai";
pub const GITHUB_LATEST_API: &str =
    "https://api.github.com/repos/k0ta0uchi/OpenRedSamurai/releases/latest";
pub const AUTOSTART_VALUE_NAME: &str = PRODUCT_NAME;
pub const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const TRAY_ARGUMENT: &str = "--tray";

const INSTALL_MARKER: &str = ".redsamurai-install";
const INSTALL_MARKER_CONTENT: &str = "redsamurai-config";
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

/// A strict three-part release version.  GitHub tags are expected to be
/// `vMAJOR.MINOR.PATCH` or `MAJOR.MINOR.PATCH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AppVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl AppVersion {
    pub const fn current() -> Self {
        // `CURRENT_VERSION` is validated by Cargo's package metadata.  The
        // fallback keeps this const constructor usable in tests and avoids a
        // panic at runtime if a downstream package uses a non-semver value.
        match parse_version_const(CURRENT_VERSION.as_bytes()) {
            Some(version) => version,
            None => Self {
                major: 0,
                minor: 0,
                patch: 0,
            },
        }
    }

    pub fn parse(value: &str) -> Result<Self, UpdateError> {
        let trimmed = value.trim().strip_prefix('v').unwrap_or(value.trim());
        let mut parts = trimmed.split('.');
        let major = parse_component(parts.next(), value)?;
        let minor = parse_component(parts.next(), value)?;
        let patch = parse_component(parts.next(), value)?;
        if parts.next().is_some() {
            return Err(UpdateError::InvalidRelease(format!(
                "version must have exactly three components: {value:?}"
            )));
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for AppVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn parse_component(part: Option<&str>, original: &str) -> Result<u32, UpdateError> {
    let Some(part) = part else {
        return Err(UpdateError::InvalidRelease(format!(
            "version must have three numeric components: {original:?}"
        )));
    };
    if part.is_empty() || (part.len() > 1 && part.starts_with('0')) {
        return Err(UpdateError::InvalidRelease(format!(
            "version component is invalid: {original:?}"
        )));
    }
    part.parse::<u32>().map_err(|_| {
        UpdateError::InvalidRelease(format!("version component is not numeric: {original:?}"))
    })
}

const fn parse_version_const(bytes: &[u8]) -> Option<AppVersion> {
    let mut values = [0u32; 3];
    let mut value_index = 0usize;
    let mut current = 0u32;
    let mut has_digit = false;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'.' {
            if !has_digit || value_index >= 2 {
                return None;
            }
            values[value_index] = current;
            value_index += 1;
            current = 0;
            has_digit = false;
        } else if byte >= b'0' && byte <= b'9' {
            current = current * 10 + (byte - b'0') as u32;
            has_digit = true;
        } else {
            return None;
        }
        index += 1;
    }
    if !has_digit || value_index != 2 {
        return None;
    }
    values[2] = current;
    Some(AppVersion {
        major: values[0],
        minor: values[1],
        patch: values[2],
    })
}

#[derive(Debug)]
pub enum UpdateError {
    Io(String),
    Network(String),
    InvalidRelease(String),
    Integrity(String),
    Archive(String),
    Registry(String),
    UnsupportedPlatform,
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "ファイル操作に失敗しました: {message}"),
            Self::Network(message) => write!(f, "GitHubへの接続に失敗しました: {message}"),
            Self::InvalidRelease(message) => write!(f, "GitHubリリースを検証できません: {message}"),
            Self::Integrity(message) => write!(f, "ダウンロード検証に失敗しました: {message}"),
            Self::Archive(message) => write!(f, "配布アーカイブを展開できません: {message}"),
            Self::Registry(message) => write!(f, "自動起動設定に失敗しました: {message}"),
            Self::UnsupportedPlatform => f.write_str("インストーラーはWindowsでのみ利用できます"),
        }
    }
}

impl std::error::Error for UpdateError {}

impl From<io::Error> for UpdateError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub version: AppVersion,
    pub package_name: String,
    pub package_url: String,
    pub checksum_url: String,
    pub html_url: String,
    pub package_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: Option<u64>,
}

/// Parse a GitHub Releases API response without performing network I/O.
/// Keeping this function public gives the installer a deterministic test seam.
pub fn parse_release_json(body: &str) -> Result<ReleaseInfo, UpdateError> {
    let release: GitHubRelease = serde_json::from_str(body)
        .map_err(|error| UpdateError::InvalidRelease(format!("JSON parse error: {error}")))?;
    if release.draft || release.prerelease {
        return Err(UpdateError::InvalidRelease(
            "draft and prerelease versions are not accepted".to_owned(),
        ));
    }
    let version = AppVersion::parse(&release.tag_name)?;
    let package_name = format!("OpenRedSamurai-v{}-windows-x64.zip", version);
    let checksum_name = format!("{package_name}.sha256");
    let package = release
        .assets
        .iter()
        .find(|asset| asset.name == package_name)
        .ok_or_else(|| UpdateError::InvalidRelease(format!("asset is missing: {package_name}")))?;
    let checksum = release
        .assets
        .iter()
        .find(|asset| asset.name == checksum_name)
        .ok_or_else(|| UpdateError::InvalidRelease(format!("asset is missing: {checksum_name}")))?;
    validate_github_asset_url(&package.browser_download_url)?;
    validate_github_asset_url(&checksum.browser_download_url)?;
    Ok(ReleaseInfo {
        tag_name: release.tag_name,
        version,
        package_name,
        package_url: package.browser_download_url.clone(),
        checksum_url: checksum.browser_download_url.clone(),
        html_url: release.html_url,
        package_size: package.size,
    })
}

fn validate_github_asset_url(url: &str) -> Result<(), UpdateError> {
    let prefix = format!("https://github.com/{GITHUB_REPOSITORY}/releases/download/");
    if !url.starts_with(&prefix) || url.contains("..") || url.contains('\n') || url.contains('\r') {
        return Err(UpdateError::InvalidRelease(format!(
            "asset URL is outside the repository: {url:?}"
        )));
    }
    Ok(())
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent("OpenRedSamurai-updater/1.0")
        .timeout(Duration::from_secs(20))
        .build()
}

/// Attach an optional read-only GitHub token without ever persisting it.
/// Public releases work without credentials. A private repository can be
/// updated when the operator supplies `OPENREDSAMURAI_GITHUB_TOKEN` (or the
/// conventional `GH_TOKEN`) in the launching user's environment.
fn github_request(request: ureq::Request) -> ureq::Request {
    let token = std::env::var("OPENREDSAMURAI_GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GH_TOKEN").ok())
        .map(|value| value.trim().to_owned());
    let Some(token) = token.filter(|value| {
        !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
    }) else {
        return request;
    };
    request.set("Authorization", &format!("Bearer {token}"))
}

pub fn fetch_latest_release() -> Result<ReleaseInfo, UpdateError> {
    let response = github_request(http_agent().get(GITHUB_LATEST_API))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let body = response
        .into_string()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    parse_release_json(&body)
}

/// Download and verify a release ZIP using the adjacent SHA-256 sidecar.
pub fn download_verified_release(
    release: &ReleaseInfo,
    destination: &Path,
) -> Result<String, UpdateError> {
    if let Some(size) = release.package_size {
        if size > MAX_PACKAGE_BYTES {
            return Err(UpdateError::Integrity(format!(
                "package is larger than {} bytes",
                MAX_PACKAGE_BYTES
            )));
        }
    }
    let parent = destination
        .parent()
        .ok_or_else(|| UpdateError::Io("download destination has no parent".to_owned()))?;
    fs::create_dir_all(parent)?;
    let partial = destination.with_extension("download.part");
    let _ = fs::remove_file(&partial);
    download_url_to_file(&release.package_url, &partial)?;

    let sidecar = github_request(http_agent().get(&release.checksum_url))
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?
        .into_string()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let expected = parse_sha256_sidecar(&sidecar, &release.package_name)?;
    let actual = sha256_file(&partial)?;
    if !actual.eq_ignore_ascii_case(&expected) {
        let _ = fs::remove_file(&partial);
        return Err(UpdateError::Integrity(format!(
            "expected {expected}, received {actual}"
        )));
    }
    replace_file(&partial, destination)?;
    Ok(actual)
}

fn download_url_to_file(url: &str, destination: &Path) -> Result<(), UpdateError> {
    validate_github_asset_url(url)?;
    let response = github_request(http_agent().get(url))
        .set("Accept", "application/octet-stream")
        .call()
        .map_err(|error| UpdateError::Network(error.to_string()))?;
    let content_length = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok());
    if content_length.is_some_and(|size| size > MAX_PACKAGE_BYTES) {
        return Err(UpdateError::Integrity(format!(
            "package is larger than {} bytes",
            MAX_PACKAGE_BYTES
        )));
    }
    let mut reader = response.into_reader();
    let mut file = File::create(destination)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| UpdateError::Network(error.to_string()))?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > MAX_PACKAGE_BYTES {
            let _ = fs::remove_file(destination);
            return Err(UpdateError::Integrity(format!(
                "package is larger than {} bytes",
                MAX_PACKAGE_BYTES
            )));
        }
        file.write_all(&buffer[..read])?;
    }
    file.flush()?;
    Ok(())
}

pub fn parse_sha256_sidecar(body: &str, package_name: &str) -> Result<String, UpdateError> {
    for line in body.lines() {
        let mut fields = line.split_whitespace();
        let Some(hash) = fields.next() else { continue };
        let Some(file_name) = fields.next() else {
            continue;
        };
        let file_name = file_name.strip_prefix('*').unwrap_or(file_name);
        if file_name == package_name
            && hash.len() == 64
            && hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Ok(hash.to_ascii_lowercase());
        }
    }
    Err(UpdateError::Integrity(format!(
        "sidecar does not contain a SHA-256 entry for {package_name}"
    )))
}

pub fn sha256_file(path: &Path) -> Result<String, UpdateError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Extract a release archive into a staging directory.  Every ZIP entry is
/// checked with `enclosed_name`; absolute paths, parent traversal and symlink
/// entries are rejected before any file is written.
pub fn extract_package(zip_path: &Path, destination: &Path) -> Result<(), UpdateError> {
    fs::create_dir_all(destination)?;
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| UpdateError::Archive(format!("ZIP open error: {error}")))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| UpdateError::Archive(format!("ZIP entry error: {error}")))?;
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(UpdateError::Archive(format!(
                "ZIP entry is larger than {} bytes: {}",
                MAX_ENTRY_BYTES,
                entry.name()
            )));
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(UpdateError::Archive(format!(
                "symbolic-link ZIP entry is not allowed: {}",
                entry.name()
            )));
        }
        let relative = entry.enclosed_name().ok_or_else(|| {
            UpdateError::Archive(format!("unsafe ZIP entry path: {}", entry.name()))
        })?;
        if relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(UpdateError::Archive(format!(
                "unsafe ZIP entry path: {}",
                entry.name()
            )));
        }
        let output = destination.join(relative);
        if !output.starts_with(destination) {
            return Err(UpdateError::Archive(format!(
                "ZIP entry escaped staging directory: {}",
                entry.name()
            )));
        }
        if entry.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output_file = File::create(&output)?;
        io::copy(&mut entry, &mut output_file)?;
        output_file.flush()?;
    }
    Ok(())
}

pub fn locate_payload_root(staging: &Path) -> Result<PathBuf, UpdateError> {
    let direct = staging.join(PRODUCT_EXECUTABLE_NAME);
    if direct.is_file() {
        return Ok(staging.to_path_buf());
    }
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join(PRODUCT_EXECUTABLE_NAME).is_file() {
            return Ok(path);
        }
    }
    Err(UpdateError::Archive(format!(
        "{PRODUCT_EXECUTABLE_NAME} is missing from the package"
    )))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOutcome {
    pub version: AppVersion,
    pub install_directory: PathBuf,
    pub data_directory: PathBuf,
    pub executable_path: PathBuf,
    pub startup_command: String,
}

pub fn default_install_directory() -> Result<PathBuf, UpdateError> {
    dirs::data_local_dir()
        .map(|path| path.join("RED SAMURAI"))
        .ok_or_else(|| UpdateError::Io("LOCALAPPDATA could not be determined".to_owned()))
}

pub fn default_data_directory() -> Result<PathBuf, UpdateError> {
    let local_app_data = dirs::data_local_dir()
        .ok_or_else(|| UpdateError::Io("LOCALAPPDATA could not be determined".to_owned()))?;
    let fallback = local_app_data.join("OpenRedSamurai").join(PRODUCT_NAME);
    let Some(documents) = dirs::document_dir() else {
        return Ok(fallback);
    };
    let candidate = documents.join(PRODUCT_NAME);
    if path_traverses_reparse_point(&candidate) {
        Ok(fallback)
    } else {
        Ok(candidate)
    }
}

fn path_traverses_reparse_point(path: &Path) -> bool {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if let Ok(metadata) = fs::symlink_metadata(candidate) {
            if metadata.file_type().is_symlink() {
                return true;
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return true;
                }
            }
        }
        current = candidate.parent();
    }
    false
}

fn validate_absolute_path(path: &Path, field: &str) -> Result<(), UpdateError> {
    if !path.is_absolute() {
        return Err(UpdateError::Io(format!("{field} must be absolute")));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(UpdateError::Io(format!("{field} must not contain '..'")));
    }
    Ok(())
}

fn validate_user_path(path: &Path, field: &str) -> Result<(), UpdateError> {
    validate_absolute_path(path, field)?;
    if path_traverses_reparse_point(path) {
        return Err(UpdateError::Io(format!(
            "{field} must not traverse a reparse point"
        )));
    }
    Ok(())
}

pub fn install_payload(
    source_directory: &Path,
    install_directory: &Path,
    data_directory: &Path,
) -> Result<InstallOutcome, UpdateError> {
    install_payload_with_version(
        source_directory,
        install_directory,
        data_directory,
        AppVersion::current(),
    )
}

pub fn install_payload_with_version(
    source_directory: &Path,
    install_directory: &Path,
    data_directory: &Path,
    version: AppVersion,
) -> Result<InstallOutcome, UpdateError> {
    // The package may legitimately be launched from a cloud-synced download
    // folder.  Source validation is read-only; only mutation targets reject
    // reparse-point traversal.
    validate_absolute_path(source_directory, "source directory")?;
    validate_user_path(install_directory, "install directory")?;
    validate_user_path(data_directory, "data directory")?;
    if same_or_child_path(data_directory, install_directory) {
        return Err(UpdateError::Io(
            "data directory must be outside the install directory".to_owned(),
        ));
    }
    let source_executable = source_directory.join(PRODUCT_EXECUTABLE_NAME);
    let source_installer = source_directory.join(INSTALLER_EXECUTABLE_NAME);
    if !source_executable.is_file() {
        return Err(UpdateError::Io(format!(
            "source executable is missing: {}",
            source_executable.display()
        )));
    }
    if !source_installer.is_file() {
        return Err(UpdateError::Io(format!(
            "source installer is missing: {}",
            source_installer.display()
        )));
    }

    fs::create_dir_all(install_directory)?;
    fs::create_dir_all(data_directory)?;
    let destination_executable = install_directory.join(PRODUCT_EXECUTABLE_NAME);
    let destination_installer = install_directory.join(INSTALLER_EXECUTABLE_NAME);
    replace_if_different(&source_executable, &destination_executable)?;
    replace_if_different(&source_installer, &destination_installer)?;
    fs::write(
        install_directory.join(INSTALL_MARKER),
        INSTALL_MARKER_CONTENT.as_bytes(),
    )?;
    fs::write(
        install_directory.join("VERSION"),
        format!("{version}\n").as_bytes(),
    )?;
    let startup_command = register_current_user_startup(&destination_executable)?;
    Ok(InstallOutcome {
        version,
        install_directory: install_directory.to_path_buf(),
        data_directory: data_directory.to_path_buf(),
        executable_path: destination_executable,
        startup_command,
    })
}

fn replace_if_different(source: &Path, destination: &Path) -> Result<(), UpdateError> {
    let source_key = fs::canonicalize(source).ok();
    let destination_key = fs::canonicalize(destination).ok();
    if source_key.is_some() && source_key == destination_key {
        return Ok(());
    }
    let temporary = destination.with_extension("new");
    let _ = fs::remove_file(&temporary);
    fs::copy(source, &temporary)?;
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(&temporary, destination)?;
    Ok(())
}

fn same_or_child_path(candidate: &Path, ancestor: &Path) -> bool {
    let candidate = fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf());
    let ancestor = fs::canonicalize(ancestor).unwrap_or_else(|_| ancestor.to_path_buf());
    candidate == ancestor || candidate.starts_with(ancestor)
}

fn register_current_user_startup(executable: &Path) -> Result<String, UpdateError> {
    let registration = crate::phase4::RunRegistration::for_executable(executable, &[TRAY_ARGUMENT])
        .map_err(|error| UpdateError::Registry(error.to_string()))?;
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (run_key, _) = hkcu
            .create_subkey(RUN_SUBKEY)
            .map_err(|error| UpdateError::Registry(error.to_string()))?;
        if let Ok(existing) = run_key.get_value::<String, _>(AUTOSTART_VALUE_NAME) {
            if existing != registration.command() {
                return Err(UpdateError::Registry(
                    "既存の自動起動値が別のインストールを指しています".to_owned(),
                ));
            }
        }
        run_key
            .set_value(AUTOSTART_VALUE_NAME, &registration.command())
            .map_err(|error| UpdateError::Registry(error.to_string()))?;
        Ok(registration.command().to_owned())
    }
    #[cfg(not(windows))]
    {
        let _ = registration;
        Err(UpdateError::UnsupportedPlatform)
    }
}

pub fn launch_installer(arguments: &[&str]) -> Result<(), UpdateError> {
    let current = std::env::current_exe()?;
    let Some(parent) = current.parent() else {
        return Err(UpdateError::Io(
            "current executable has no parent".to_owned(),
        ));
    };
    let installer = parent.join(INSTALLER_EXECUTABLE_NAME);
    if !installer.is_file() {
        return Err(UpdateError::Io(format!(
            "installer executable is missing: {}",
            installer.display()
        )));
    }
    std::process::Command::new(installer)
        .args(arguments)
        .spawn()
        .map(|_| ())
        .map_err(UpdateError::from)
}

pub fn uninstall_current_user(
    install_directory: &Path,
    data_directory: &Path,
) -> Result<(), UpdateError> {
    validate_user_path(install_directory, "install directory")?;
    validate_user_path(data_directory, "data directory")?;
    let marker = install_directory.join(INSTALL_MARKER);
    if fs::read(&marker).ok().as_deref() != Some(INSTALL_MARKER_CONTENT.as_bytes()) {
        return Err(UpdateError::Io(
            "trusted install marker is missing or invalid".to_owned(),
        ));
    }
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey(RUN_SUBKEY) {
            if let Ok(existing) = run_key.get_value::<String, _>(AUTOSTART_VALUE_NAME) {
                let executable = install_directory.join(PRODUCT_EXECUTABLE_NAME);
                let expected =
                    crate::phase4::RunRegistration::for_executable(&executable, &[TRAY_ARGUMENT])
                        .map_err(|error| UpdateError::Registry(error.to_string()))?;
                if existing != expected.command() {
                    return Err(UpdateError::Registry(
                        "既存の自動起動値が別のインストールを指しています".to_owned(),
                    ));
                }
            }
        }
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_SUBKEY, winreg::enums::KEY_WRITE) {
            let _ = run_key.delete_value(AUTOSTART_VALUE_NAME);
        }
    }
    #[cfg(not(windows))]
    {
        return Err(UpdateError::UnsupportedPlatform);
    }
    fs::remove_dir_all(install_directory)?;
    // Product data is intentionally retained.  Keep the argument meaningful
    // so callers must make that boundary explicit.
    let _ = data_directory;
    Ok(())
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), UpdateError> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(source, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn release_json(tag: &str, assets: &str) -> String {
        format!(
            r#"{{
                "tag_name": "{tag}",
                "html_url": "https://github.com/k0ta0uchi/OpenRedSamurai/releases/tag/{tag}",
                "draft": false,
                "prerelease": false,
                "assets": [{assets}]
            }}"#
        )
    }

    #[test]
    fn versions_sort_numerically_and_accept_v_prefix() {
        assert!(AppVersion::parse("v1.10.0").unwrap() > AppVersion::parse("1.9.9").unwrap());
        assert_eq!(
            AppVersion::current(),
            AppVersion::parse(CURRENT_VERSION).unwrap()
        );
    }

    #[test]
    fn release_parser_requires_expected_zip_and_sidecar() {
        let body = release_json(
            "v2.3.4",
            r#"
                {"name":"OpenRedSamurai-v2.3.4-windows-x64.zip","browser_download_url":"https://github.com/k0ta0uchi/OpenRedSamurai/releases/download/v2.3.4/OpenRedSamurai-v2.3.4-windows-x64.zip","size":123},
                {"name":"OpenRedSamurai-v2.3.4-windows-x64.zip.sha256","browser_download_url":"https://github.com/k0ta0uchi/OpenRedSamurai/releases/download/v2.3.4/OpenRedSamurai-v2.3.4-windows-x64.zip.sha256","size":100}
            "#,
        );
        let release = parse_release_json(&body).unwrap();
        assert_eq!(
            release.version,
            AppVersion {
                major: 2,
                minor: 3,
                patch: 4
            }
        );
        assert_eq!(release.package_size, Some(123));
    }

    #[test]
    fn release_parser_rejects_external_asset_urls() {
        let body = release_json(
            "v2.3.4",
            r#"
                {"name":"OpenRedSamurai-v2.3.4-windows-x64.zip","browser_download_url":"https://example.invalid/package.zip","size":123},
                {"name":"OpenRedSamurai-v2.3.4-windows-x64.zip.sha256","browser_download_url":"https://github.com/k0ta0uchi/OpenRedSamurai/releases/download/v2.3.4/OpenRedSamurai-v2.3.4-windows-x64.zip.sha256","size":100}
            "#,
        );
        assert!(parse_release_json(&body).is_err());
    }

    #[test]
    fn sidecar_parser_accepts_binary_and_text_forms() {
        let hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(
            parse_sha256_sidecar(&format!("{hash}  package.zip\n"), "package.zip").unwrap(),
            hash
        );
        assert_eq!(
            parse_sha256_sidecar(&format!("{hash} *package.zip\n"), "package.zip").unwrap(),
            hash
        );
        assert!(parse_sha256_sidecar(&format!("{hash}  other.zip\n"), "package.zip").is_err());
    }

    #[test]
    fn extraction_rejects_parent_traversal_and_keeps_payload_inside_stage() {
        let root =
            std::env::temp_dir().join(format!("redsamurai-installer-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let zip_path = root.join("unsafe.zip");
        {
            let file = File::create(&zip_path).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("../escape.txt", options).unwrap();
            writer.write_all(b"bad").unwrap();
            writer.finish().unwrap();
        }
        let result = extract_package(&zip_path, &root.join("stage"));
        assert!(result.is_err());
        assert!(!root.join("escape.txt").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn install_and_uninstall_register_current_user_and_preserve_data() {
        use std::time::{SystemTime, UNIX_EPOCH};
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        struct RunValueGuard {
            previous: Option<String>,
        }

        impl Drop for RunValueGuard {
            fn drop(&mut self) {
                let hkcu = RegKey::predef(HKEY_CURRENT_USER);
                let Ok((run_key, _)) = hkcu.create_subkey(RUN_SUBKEY) else {
                    return;
                };
                match &self.previous {
                    Some(value) => {
                        let _ = run_key.set_value(AUTOSTART_VALUE_NAME, value);
                    }
                    None => {
                        let _ = run_key.delete_value(AUTOSTART_VALUE_NAME);
                    }
                }
            }
        }

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (run_key, _) = hkcu.create_subkey(RUN_SUBKEY).unwrap();
        let previous = run_key.get_value::<String, _>(AUTOSTART_VALUE_NAME).ok();
        let guard = RunValueGuard { previous };
        let _ = run_key.delete_value(AUTOSTART_VALUE_NAME);

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "redsamurai-installer-live-{}-{nonce}",
            std::process::id()
        ));
        let source = root.join("source");
        let install = root.join("install");
        let data = root.join("data");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join(PRODUCT_EXECUTABLE_NAME), b"editor payload").unwrap();
        fs::write(source.join(INSTALLER_EXECUTABLE_NAME), b"installer payload").unwrap();
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("profile.pfd"), b"profile bytes").unwrap();

        let outcome = install_payload_with_version(
            &source,
            &install,
            &data,
            AppVersion {
                major: 2,
                minor: 4,
                patch: 6,
            },
        )
        .unwrap();
        assert_eq!(
            outcome.version,
            AppVersion {
                major: 2,
                minor: 4,
                patch: 6
            }
        );
        assert_eq!(
            fs::read(install.join(PRODUCT_EXECUTABLE_NAME)).unwrap(),
            b"editor payload"
        );
        assert_eq!(
            fs::read(install.join(INSTALLER_EXECUTABLE_NAME)).unwrap(),
            b"installer payload"
        );
        assert_eq!(
            fs::read_to_string(install.join("VERSION")).unwrap(),
            "2.4.6\n"
        );
        assert_eq!(
            run_key
                .get_value::<String, _>(AUTOSTART_VALUE_NAME)
                .unwrap(),
            outcome.startup_command
        );

        uninstall_current_user(&install, &data).unwrap();
        assert!(!install.exists());
        assert_eq!(
            fs::read(data.join("profile.pfd")).unwrap(),
            b"profile bytes"
        );
        drop(guard);
        let _ = fs::remove_dir_all(root);
    }
}
