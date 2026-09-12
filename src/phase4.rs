//! Typed, side-effect-free integration plans for Phase 4.
//!
//! This module deliberately models tray/autostart/installer work without
//! touching the registry or launching an installer.  The PowerShell scripts
//! in `installer/` are the explicit execution boundary; callers can inspect
//! and present the plans before choosing to run those scripts.

use std::env;
use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};

/// Canonical product name used by the profile directory and the Run value.
/// Keep this in one place: changing it would orphan existing profiles and
/// leave a stale autostart entry behind.
pub const PRODUCT_NAME: &str = "RED SAMURAI 16400DPI Gaming Mouse";
/// Stable human-facing title for installer and tray integrations.
pub const PRODUCT_DISPLAY_NAME: &str = "RED SAMURAI Gaming Mouse Configuration";
/// Stable executable filename expected by the installer scripts.
pub const PRODUCT_EXECUTABLE_NAME: &str = "redsamurai-config.exe";
/// Stable product identifier for manifests and future installer metadata.
pub const PRODUCT_ID: &str = "redsamurai-config";
/// The data directory leaf below the user's Documents directory.
pub const DATA_DIRECTORY_NAME: &str = PRODUCT_NAME;
/// The per-user Run value name.  It intentionally does not include a version.
pub const AUTOSTART_VALUE_NAME: &str = PRODUCT_NAME;
/// Registry path used by the concrete PowerShell installer (HKCU only).
pub const RUN_REGISTRY_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const RUN_REGISTRY_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
/// Argument supplied to a startup process so it can select tray mode.
pub const TRAY_ARGUMENT: &str = "--tray";

/// Errors raised before any platform integration side effect can occur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase4Error {
    /// Registry integration is available only in a Windows execution
    /// environment.  Plan construction itself remains cross-platform so it
    /// can be reviewed and tested as a dry run.
    UnsupportedPlatform,
    /// A machine-wide operation was requested without an explicit elevation
    /// boundary.  Phase 4 only generates per-user plans.
    ElevationRequired { operation: &'static str },
    /// The current user profile could not be discovered.
    MissingUserProfile,
    /// A required path was empty.
    EmptyPath { field: &'static str },
    /// A path must be absolute before it is placed in a startup command or a
    /// file operation.
    RelativePath { field: &'static str, path: PathBuf },
    /// Parent components are rejected instead of being normalized away.  The
    /// installer therefore cannot escape the path selected by its caller.
    PathTraversal { path: PathBuf },
    /// The executable path is absolute but does not identify a file-like leaf.
    InvalidExecutablePath { path: PathBuf },
    /// Uninstall must not recursively remove the directory that contains user
    /// data (or a child of it).
    DataDirectoryInsideInstallDirectory {
        install_directory: PathBuf,
        data_directory: PathBuf,
    },
    /// NUL and other invalid command-line values are rejected before a Run
    /// string is generated.
    InvalidArgument {
        argument: String,
        reason: &'static str,
    },
    /// `apply` is intentionally not implemented by this side-effect-free
    /// module.  The reviewed PowerShell scripts are the execution boundary.
    DryRunOnly,
}

impl fmt::Display for Phase4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("Phase 4 integration is Windows-only"),
            Self::ElevationRequired { operation } => {
                write!(f, "{operation} requires explicit elevation")
            }
            Self::MissingUserProfile => {
                f.write_str("the current user profile could not be discovered")
            }
            Self::EmptyPath { field } => write!(f, "{field} must not be empty"),
            Self::RelativePath { field, path } => {
                write!(f, "{field} must be absolute: {}", path.display())
            }
            Self::PathTraversal { path } => {
                write!(f, "path traversal is not allowed: {}", path.display())
            }
            Self::InvalidExecutablePath { path } => {
                write!(f, "invalid executable path: {}", path.display())
            }
            Self::DataDirectoryInsideInstallDirectory {
                install_directory,
                data_directory,
            } => write!(
                f,
                "data directory {} is inside install directory {}",
                data_directory.display(),
                install_directory.display()
            ),
            Self::InvalidArgument { argument, reason } => {
                write!(f, "invalid command-line argument {argument:?}: {reason}")
            }
            Self::DryRunOnly => {
                f.write_str("Phase 4 plans are dry-run only; use installer scripts to apply them")
            }
        }
    }
}

impl Error for Phase4Error {}

/// Target scope for an installer plan.  Current-user scope is the only scope
/// that can be generated without an explicit administrative boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallScope {
    CurrentUser,
    AllUsers,
}

impl InstallScope {
    pub const fn is_machine_wide(self) -> bool {
        matches!(self, Self::AllUsers)
    }
}

impl Default for InstallScope {
    fn default() -> Self {
        Self::CurrentUser
    }
}

/// Inputs to [`InstallPlan::build`].  All paths are validated when the plan
/// is built, not when this value is assembled, which keeps the type useful to
/// UI forms that collect values incrementally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOptions {
    pub executable: PathBuf,
    pub scope: InstallScope,
    pub install_directory: Option<PathBuf>,
    pub data_directory: Option<PathBuf>,
    pub register_autostart: bool,
    pub launch_arguments: Vec<String>,
}

impl InstallOptions {
    /// Build a safe current-user configuration with tray startup enabled.
    pub fn current_user(executable: impl AsRef<Path>) -> Self {
        Self {
            executable: executable.as_ref().to_path_buf(),
            scope: InstallScope::CurrentUser,
            install_directory: None,
            data_directory: None,
            register_autostart: true,
            launch_arguments: vec![TRAY_ARGUMENT.to_string()],
        }
    }

    /// Alias that reads naturally at call sites handling multiple scopes.
    pub fn for_current_user(executable: impl AsRef<Path>) -> Self {
        Self::current_user(executable)
    }

    /// Construct a request for machine-wide installation.  Building it fails
    /// with [`Phase4Error::ElevationRequired`] rather than silently writing to
    /// HKLM or changing the caller's security context.
    pub fn all_users(executable: impl AsRef<Path>) -> Self {
        let mut options = Self::current_user(executable);
        options.scope = InstallScope::AllUsers;
        options
    }

    pub fn with_install_directory(mut self, path: impl AsRef<Path>) -> Self {
        self.install_directory = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_data_directory(mut self, path: impl AsRef<Path>) -> Self {
        self.data_directory = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_autostart(mut self, enabled: bool) -> Self {
        self.register_autostart = enabled;
        self
    }

    pub fn without_autostart(self) -> Self {
        self.with_autostart(false)
    }

    pub fn with_launch_arguments<I, S>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.launch_arguments = arguments.into_iter().map(Into::into).collect();
        self
    }
}

/// A complete per-user Run registration generated from an executable path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRegistration {
    value_name: String,
    command: String,
}

impl RunRegistration {
    pub fn new(executable: impl AsRef<Path>, arguments: &[&str]) -> Result<Self, Phase4Error> {
        Self::for_executable(executable, arguments)
    }

    pub fn for_executable(
        executable: impl AsRef<Path>,
        arguments: &[&str],
    ) -> Result<Self, Phase4Error> {
        Ok(Self {
            value_name: AUTOSTART_VALUE_NAME.to_string(),
            command: build_autostart_command(executable, arguments)?,
        })
    }

    pub fn value_name(&self) -> &str {
        &self.value_name
    }

    pub fn command(&self) -> &str {
        &self.command
    }
}

/// Alias that makes the registry scope explicit for consumers that prefer it.
pub type PerUserRunRegistration = RunRegistration;

/// One declarative installer action.  None of these variants performs I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallerOperation {
    EnsureDirectory {
        path: PathBuf,
    },
    RegisterCurrentUserRun {
        value_name: String,
        command: String,
    },
    RemoveCurrentUserRun {
        value_name: String,
    },
    RemoveInstalledFiles {
        directory: PathBuf,
    },
    /// User profile data is deliberately retained by uninstall.
    PreserveDataDirectory {
        path: PathBuf,
    },
}

/// Short alias for callers that refer to operations as plan steps.
pub type InstallerStep = InstallerOperation;

impl InstallerOperation {
    pub fn is_idempotent(&self) -> bool {
        true
    }
}

/// Dry-run installation plan for the current user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    scope: InstallScope,
    executable: PathBuf,
    install_directory: PathBuf,
    data_directory: PathBuf,
    autostart: Option<RunRegistration>,
    operations: Vec<InstallerOperation>,
}

impl InstallPlan {
    pub fn build(options: InstallOptions) -> Result<Self, Phase4Error> {
        if options.scope.is_machine_wide() {
            return Err(Phase4Error::ElevationRequired {
                operation: "all-users installation",
            });
        }

        let executable = validate_executable_path(&options.executable)?;
        let install_directory = match options.install_directory.as_deref() {
            Some(path) => validate_path(path, "install directory")?,
            None => executable_parent(&executable)
                .ok_or_else(|| Phase4Error::InvalidExecutablePath {
                    path: executable.clone(),
                })
                .and_then(|path| validate_path(&path, "install directory"))?,
        };
        let data_directory = match options.data_directory.as_deref() {
            Some(path) => validate_path(path, "data directory")?,
            None => discover_data_directory()?,
        };
        if is_same_or_child_path(&data_directory, &install_directory) {
            return Err(Phase4Error::DataDirectoryInsideInstallDirectory {
                install_directory,
                data_directory,
            });
        }
        // The Run value must launch the copy owned by this install plan.  If a
        // caller supplies a separate install directory, accepting an
        // executable from another tree would register a command that install
        // and uninstall cannot account for deterministically.
        let executable_directory =
            executable_parent(&executable).ok_or_else(|| Phase4Error::InvalidExecutablePath {
                path: executable.clone(),
            })?;
        if !is_same_path(&executable_directory, &install_directory) {
            return Err(Phase4Error::InvalidExecutablePath { path: executable });
        }

        let autostart = if options.register_autostart {
            validate_current_user_autostart_arguments(&options.launch_arguments)?;
            let argument_refs: Vec<&str> = options
                .launch_arguments
                .iter()
                .map(String::as_str)
                .collect();
            Some(RunRegistration::for_executable(
                &executable,
                &argument_refs,
            )?)
        } else {
            None
        };

        let mut operations = Vec::with_capacity(3);
        push_unique(
            &mut operations,
            InstallerOperation::EnsureDirectory {
                path: install_directory.clone(),
            },
        );
        push_unique(
            &mut operations,
            InstallerOperation::EnsureDirectory {
                path: data_directory.clone(),
            },
        );
        if let Some(registration) = &autostart {
            push_unique(
                &mut operations,
                InstallerOperation::RegisterCurrentUserRun {
                    value_name: registration.value_name.clone(),
                    command: registration.command.clone(),
                },
            );
        }

        Ok(Self {
            scope: options.scope,
            executable,
            install_directory,
            data_directory,
            autostart,
            operations,
        })
    }

    pub fn for_current_user(executable: impl AsRef<Path>) -> Result<Self, Phase4Error> {
        Self::build(InstallOptions::current_user(executable))
    }

    pub fn generate(options: InstallOptions) -> Result<Self, Phase4Error> {
        Self::build(options)
    }

    pub fn new(executable: impl AsRef<Path>) -> Result<Self, Phase4Error> {
        Self::for_current_user(executable)
    }

    pub fn scope(&self) -> InstallScope {
        self.scope
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn install_directory(&self) -> &Path {
        &self.install_directory
    }

    pub fn data_directory(&self) -> &Path {
        &self.data_directory
    }

    pub fn autostart(&self) -> Option<&RunRegistration> {
        self.autostart.as_ref()
    }

    pub fn autostart_value_name(&self) -> &str {
        AUTOSTART_VALUE_NAME
    }

    pub fn autostart_command(&self) -> Option<&str> {
        self.autostart.as_ref().map(RunRegistration::command)
    }

    pub fn operations(&self) -> &[InstallerOperation] {
        &self.operations
    }

    pub fn steps(&self) -> &[InstallerOperation] {
        self.operations()
    }

    /// Repeating the same script is safe because each operation has an
    /// idempotent meaning and the builder removes duplicates.
    pub fn is_idempotent(&self) -> bool {
        operations_are_idempotent(&self.operations)
    }

    /// Registry APIs are intentionally not embedded in this module.  Calling
    /// this method can never mutate the machine; use `installer/install.ps1`
    /// after reviewing the returned plan.
    pub fn apply(&self) -> Result<(), Phase4Error> {
        require_windows()?;
        Err(Phase4Error::DryRunOnly)
    }
}

/// Dry-run uninstallation plan.  User profile data is never removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallPlan {
    scope: InstallScope,
    executable: PathBuf,
    install_directory: PathBuf,
    data_directory: PathBuf,
    operations: Vec<InstallerOperation>,
}

impl UninstallPlan {
    pub fn build(scope: InstallScope, executable: impl AsRef<Path>) -> Result<Self, Phase4Error> {
        Self::build_internal(scope, executable.as_ref(), None)
    }

    pub fn build_with_data_directory(
        scope: InstallScope,
        executable: impl AsRef<Path>,
        data_directory: impl AsRef<Path>,
    ) -> Result<Self, Phase4Error> {
        Self::build_internal(scope, executable.as_ref(), Some(data_directory.as_ref()))
    }

    fn build_internal(
        scope: InstallScope,
        executable_path: &Path,
        data_directory: Option<&Path>,
    ) -> Result<Self, Phase4Error> {
        if scope.is_machine_wide() {
            return Err(Phase4Error::ElevationRequired {
                operation: "all-users uninstallation",
            });
        }
        let executable = validate_executable_path(executable_path)?;
        let install_directory = executable_parent(&executable)
            .ok_or_else(|| Phase4Error::InvalidExecutablePath {
                path: executable.clone(),
            })
            .and_then(|path| validate_path(&path, "install directory"))?;
        let data_directory = match data_directory {
            Some(path) => validate_path(path, "data directory")?,
            None => discover_data_directory()?,
        };
        if is_same_or_child_path(&data_directory, &install_directory) {
            return Err(Phase4Error::DataDirectoryInsideInstallDirectory {
                install_directory,
                data_directory,
            });
        }

        let operations = vec![
            InstallerOperation::RemoveCurrentUserRun {
                value_name: AUTOSTART_VALUE_NAME.to_string(),
            },
            InstallerOperation::RemoveInstalledFiles {
                directory: install_directory.clone(),
            },
            InstallerOperation::PreserveDataDirectory {
                path: data_directory.clone(),
            },
        ];

        Ok(Self {
            scope,
            executable,
            install_directory,
            data_directory,
            operations,
        })
    }

    pub fn for_current_user(executable: impl AsRef<Path>) -> Result<Self, Phase4Error> {
        Self::build(InstallScope::CurrentUser, executable)
    }

    pub fn for_current_user_with_data_directory(
        executable: impl AsRef<Path>,
        data_directory: impl AsRef<Path>,
    ) -> Result<Self, Phase4Error> {
        Self::build_with_data_directory(InstallScope::CurrentUser, executable, data_directory)
    }

    pub fn generate(
        scope: InstallScope,
        executable: impl AsRef<Path>,
    ) -> Result<Self, Phase4Error> {
        Self::build(scope, executable)
    }

    pub fn scope(&self) -> InstallScope {
        self.scope
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn install_directory(&self) -> &Path {
        &self.install_directory
    }

    pub fn data_directory(&self) -> &Path {
        &self.data_directory
    }

    pub fn autostart_value_name(&self) -> &str {
        AUTOSTART_VALUE_NAME
    }

    pub fn operations(&self) -> &[InstallerOperation] {
        &self.operations
    }

    pub fn steps(&self) -> &[InstallerOperation] {
        self.operations()
    }

    pub fn is_idempotent(&self) -> bool {
        operations_are_idempotent(&self.operations)
    }

    pub fn apply(&self) -> Result<(), Phase4Error> {
        require_windows()?;
        Err(Phase4Error::DryRunOnly)
    }
}

/// Convenience free function for callers that do not need to retain options.
pub fn build_install_plan(options: InstallOptions) -> Result<InstallPlan, Phase4Error> {
    InstallPlan::build(options)
}

pub fn build_uninstall_plan(executable: impl AsRef<Path>) -> Result<UninstallPlan, Phase4Error> {
    UninstallPlan::for_current_user(executable)
}

/// Generate a Windows per-user Run command.  The executable is always quoted;
/// simple arguments remain readable while arguments containing whitespace or
/// quotes use the same escaping rules as `CommandLineToArgvW`.
pub fn build_autostart_command(
    executable: impl AsRef<Path>,
    arguments: &[&str],
) -> Result<String, Phase4Error> {
    let executable = validate_executable_path(executable.as_ref())?;
    let executable_text = path_text(&executable);
    reject_nul(&executable_text, "executable path")?;

    let mut command = quote_windows_arg(&executable_text);
    for argument in arguments {
        reject_nul(argument, "command-line argument")?;
        command.push(' ');
        command.push_str(&format_windows_arg(argument));
    }
    Ok(command)
}

pub fn build_run_command(
    executable: impl AsRef<Path>,
    arguments: &[&str],
) -> Result<String, Phase4Error> {
    build_autostart_command(executable, arguments)
}

/// Quote one argument using the Windows CRT/`CommandLineToArgvW` convention.
/// The function always emits a pair of quotes, which makes it suitable for an
/// executable path in a registry Run value even when the path has no spaces.
pub fn quote_windows_arg(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    let mut backslashes = 0usize;

    for character in value.chars() {
        if character == '\\' {
            backslashes += 1;
            continue;
        }

        if character == '"' {
            for _ in 0..(backslashes * 2 + 1) {
                quoted.push('\\');
            }
            quoted.push('"');
        } else {
            for _ in 0..backslashes {
                quoted.push('\\');
            }
            quoted.push(character);
        }
        backslashes = 0;
    }

    // Backslashes immediately before the closing quote must be doubled.
    for _ in 0..(backslashes * 2) {
        quoted.push('\\');
    }
    quoted.push('"');
    quoted
}

pub fn quote_windows_command_arg(value: &str) -> String {
    quote_windows_arg(value)
}

/// Checked form for callers that accept arbitrary external input.
pub fn checked_quote_windows_arg(value: &str) -> Result<String, Phase4Error> {
    reject_nul(value, "command-line argument")?;
    Ok(quote_windows_arg(value))
}

fn format_windows_arg(value: &str) -> String {
    if !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c == '"') {
        value.to_string()
    } else {
        quote_windows_arg(value)
    }
}

fn validate_current_user_autostart_arguments(arguments: &[String]) -> Result<(), Phase4Error> {
    if arguments.len() == 1 && arguments[0] == TRAY_ARGUMENT {
        return Ok(());
    }

    Err(Phase4Error::InvalidArgument {
        argument: arguments.join(" "),
        reason: "current-user autostart requires exactly --tray",
    })
}

/// Discover the user's Documents-backed product data directory without
/// creating it.  `USERPROFILE` is preferred on Windows; `HOME` is a useful
/// deterministic fallback for dry-run tooling and non-Windows CI.
pub fn discover_data_directory() -> Result<PathBuf, Phase4Error> {
    let documents = discover_documents_directory()?;
    data_directory_from_documents(documents)
}

pub fn discover_documents_directory() -> Result<PathBuf, Phase4Error> {
    let profile = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or(Phase4Error::MissingUserProfile)?;
    let profile = validate_path(&profile, "user profile")?;
    validate_path(
        &append_path_component(&profile, "Documents"),
        "documents directory",
    )
}

pub fn data_directory_from_documents(
    documents_directory: impl AsRef<Path>,
) -> Result<PathBuf, Phase4Error> {
    let documents = validate_path(documents_directory.as_ref(), "documents directory")?;
    validate_path(
        &append_path_component(&documents, DATA_DIRECTORY_NAME),
        "data directory",
    )
}

pub fn profile_data_directory() -> Result<PathBuf, Phase4Error> {
    discover_data_directory()
}

/// This is the only platform gate exposed by the module.  Plan generation is
/// intentionally available everywhere, while execution remains explicit.
pub fn require_windows() -> Result<(), Phase4Error> {
    if cfg!(windows) {
        Ok(())
    } else {
        Err(Phase4Error::UnsupportedPlatform)
    }
}

fn reject_nul(value: &str, field: &'static str) -> Result<(), Phase4Error> {
    if value.chars().any(|character| character == '\0') {
        return Err(Phase4Error::InvalidArgument {
            argument: value.to_string(),
            reason: "NUL is not valid in a Windows command line",
        });
    }
    if value.is_empty() && field == "executable path" {
        return Err(Phase4Error::EmptyPath { field });
    }
    Ok(())
}

fn validate_executable_path(path: &Path) -> Result<PathBuf, Phase4Error> {
    let path = validate_path(path, "executable path")?;
    let Some(file_name) = path_file_name(&path) else {
        return Err(Phase4Error::InvalidExecutablePath { path });
    };
    if !is_valid_windows_file_name(&file_name) {
        return Err(Phase4Error::InvalidExecutablePath { path });
    }
    Ok(path)
}

fn validate_path(path: &Path, field: &'static str) -> Result<PathBuf, Phase4Error> {
    if path.as_os_str().is_empty() {
        return Err(Phase4Error::EmptyPath { field });
    }
    let path_text = path_text(path);
    reject_nul(&path_text, field)?;
    if has_parent_component(path) || has_current_component(path) {
        return Err(Phase4Error::PathTraversal {
            path: path.to_path_buf(),
        });
    }
    if !is_absolute_path(path) {
        return Err(Phase4Error::RelativePath {
            field,
            path: path.to_path_buf(),
        });
    }
    Ok(path.to_path_buf())
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn has_parent_component(path: &Path) -> bool {
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return true;
    }
    // A Windows path is occasionally represented as one `Normal` component
    // when this module is tested on Unix.  Inspect both separators so the
    // same traversal rule applies in dry-run CI.
    path_text(path)
        .split(['\\', '/'])
        .any(|component| component == "..")
}

fn has_current_component(path: &Path) -> bool {
    if path
        .components()
        .any(|component| component == Component::CurDir)
    {
        return true;
    }
    path_text(path)
        .split(['\\', '/'])
        .any(|component| component == ".")
}

fn is_valid_windows_file_name(file_name: &str) -> bool {
    if file_name.is_empty() || file_name == "." || file_name == ".." {
        return false;
    }
    if file_name.ends_with([' ', '.'])
        || file_name
            .chars()
            .any(|character| matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return false;
    }

    let device_stem = file_name
        .trim_end_matches([' ', '.'])
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !matches!(
        device_stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn is_absolute_path(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let text = path_text(path);
    if text.starts_with(r"\\") || text.starts_with("//") {
        return true;
    }
    let bytes = text.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn is_windows_style_path(path: &Path) -> bool {
    let text = path_text(path);
    text.starts_with(r"\\")
        || text.starts_with("//")
        || (text.len() >= 3
            && text.as_bytes()[0].is_ascii_alphabetic()
            && text.as_bytes()[1] == b':'
            && (text.as_bytes()[2] == b'\\' || text.as_bytes()[2] == b'/'))
}

/// `Path` follows the host platform's separators.  Plans are also reviewed
/// on non-Windows hosts, so understand a Windows-looking path explicitly.
fn executable_parent(path: &Path) -> Option<PathBuf> {
    if !is_windows_style_path(path) {
        return path.parent().map(Path::to_path_buf);
    }
    let text = path_text(path);
    let separator = text.rfind(['\\', '/'])?;
    if separator == 2 && text.as_bytes().get(1) == Some(&b':') {
        return Some(PathBuf::from(&text[..=separator]));
    }
    if separator == 0 {
        return Some(PathBuf::from(&text[..=separator]));
    }
    Some(PathBuf::from(&text[..separator]))
}

fn path_file_name(path: &Path) -> Option<String> {
    if is_windows_style_path(path) {
        return path_text(path)
            .trim_end_matches(['\\', '/'])
            .rsplit(['\\', '/'])
            .next()
            .filter(|name| !name.is_empty())
            .map(str::to_string);
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
}

fn append_path_component(path: &Path, component: &str) -> PathBuf {
    if is_windows_style_path(path) {
        let mut text = path_text(path);
        if !text.ends_with(['\\', '/']) {
            text.push('\\');
        }
        text.push_str(component);
        PathBuf::from(text)
    } else {
        path.join(component)
    }
}

fn is_same_or_child_path(candidate: &Path, ancestor: &Path) -> bool {
    let candidate = path_text(candidate)
        .trim_end_matches(['\\', '/'])
        .replace('/', "\\")
        .to_ascii_lowercase();
    let ancestor = ancestor
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .replace('/', "\\")
        .to_ascii_lowercase();
    candidate == ancestor || candidate.starts_with(&(ancestor + "\\"))
}

fn is_same_path(left: &Path, right: &Path) -> bool {
    let normalize = |path: &Path| {
        path_text(path)
            .trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_ascii_lowercase()
    };
    normalize(left) == normalize(right)
}

fn push_unique(operations: &mut Vec<InstallerOperation>, operation: InstallerOperation) {
    if !operations.contains(&operation) {
        operations.push(operation);
    }
}

fn operations_are_idempotent(operations: &[InstallerOperation]) -> bool {
    operations.iter().enumerate().all(|(index, operation)| {
        operation.is_idempotent() && !operations[..index].contains(operation)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn stable_product_names_are_shared_by_profiles_and_installer() {
        assert_eq!(PRODUCT_NAME, "RED SAMURAI 16400DPI Gaming Mouse");
        assert_eq!(AUTOSTART_VALUE_NAME, PRODUCT_NAME);
        assert_eq!(DATA_DIRECTORY_NAME, PRODUCT_NAME);
        assert!(!PRODUCT_DISPLAY_NAME.is_empty());
    }

    #[test]
    fn executable_and_arguments_are_quoted_for_windows_run_command() {
        let command = build_autostart_command(
            Path::new(r"C:\Program Files\RED SAMURAI\redsamurai-config.exe"),
            &["--tray"],
        )
        .expect("valid executable path");

        assert_eq!(
            command,
            r#""C:\Program Files\RED SAMURAI\redsamurai-config.exe" --tray"#
        );
    }

    #[test]
    fn quoting_escapes_trailing_backslashes_and_embedded_quotes() {
        assert_eq!(quote_windows_arg(r#"C:\a\"#), r#""C:\a\\""#);
        assert_eq!(quote_windows_arg(r#"say "hello""#), r#""say \"hello\"""#);
        assert!(matches!(
            checked_quote_windows_arg("bad\0argument"),
            Err(Phase4Error::InvalidArgument { .. })
        ));
    }

    #[test]
    fn install_plan_is_deterministic_and_does_not_duplicate_operations() {
        let executable = Path::new(r"C:\Apps\RED SAMURAI\redsamurai-config.exe");
        let first = InstallPlan::for_current_user(executable).expect("valid path");
        let second = InstallPlan::for_current_user(executable).expect("valid path");

        assert_eq!(first, second);
        assert!(first.is_idempotent());
        assert_eq!(
            first
                .operations()
                .iter()
                .filter(|operation| matches!(
                    operation,
                    InstallerOperation::RegisterCurrentUserRun { .. }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn install_and_uninstall_plans_use_the_same_run_value() {
        let executable = Path::new(r"C:\Apps\RED SAMURAI\redsamurai-config.exe");
        let install = InstallPlan::for_current_user(executable).expect("valid path");
        let uninstall = UninstallPlan::for_current_user(executable).expect("valid path");

        assert_eq!(
            install.autostart_value_name(),
            uninstall.autostart_value_name()
        );
        assert!(uninstall
            .operations()
            .iter()
            .any(|operation| matches!(operation, InstallerOperation::RemoveCurrentUserRun { .. })));
        assert!(uninstall.is_idempotent());
    }

    #[test]
    fn traversal_is_rejected_before_a_plan_is_created() {
        let executable = Path::new(r"C:\Apps\RED SAMURAI\..\evil.exe");
        let error = InstallPlan::for_current_user(executable).expect_err("traversal must fail");

        assert!(matches!(error, Phase4Error::PathTraversal { .. }));
    }

    #[test]
    fn install_plan_rejects_data_inside_install_directory() {
        let options = InstallOptions::current_user(Path::new(r"C:\Build\redsamurai-config.exe"))
            .with_install_directory(r"C:\Users\tester\Documents\RED SAMURAI")
            .with_data_directory(r"C:\Users\tester\Documents\RED SAMURAI\profiles");

        let error = InstallPlan::build(options)
            .expect_err("install must not be able to remove its data directory");
        assert_eq!(
            error.to_string(),
            r"data directory C:\Users\tester\Documents\RED SAMURAI\profiles is inside install directory C:\Users\tester\Documents\RED SAMURAI"
        );
        assert!(matches!(
            error,
            Phase4Error::DataDirectoryInsideInstallDirectory { .. }
        ));
    }

    #[test]
    fn executable_path_rejects_dot_components_and_invalid_file_names() {
        for executable in [
            r"C:\Apps\RED SAMURAI\.\redsamurai-config.exe",
            r"C:\Apps\RED SAMURAI\redsamurai?.exe",
            r"C:\Apps\RED SAMURAI\CON.exe",
        ] {
            let error = InstallPlan::for_current_user(Path::new(executable))
                .expect_err("unsafe executable path must fail before planning");
            assert!(matches!(
                error,
                Phase4Error::PathTraversal { .. } | Phase4Error::InvalidExecutablePath { .. }
            ));
        }
    }

    #[test]
    fn data_directory_is_under_the_documents_directory() {
        let documents = Path::new(r"C:\Users\tester\Documents");
        let data = data_directory_from_documents(documents).expect("valid documents path");

        assert_eq!(
            data,
            Path::new(r"C:\Users\tester\Documents\RED SAMURAI 16400DPI Gaming Mouse")
        );
    }

    #[test]
    fn all_users_scope_fails_explicitly_instead_of_silently_elevating() {
        let options = InstallOptions::all_users(Path::new(
            r"C:\Program Files\RED SAMURAI\redsamurai-config.exe",
        ));

        assert!(matches!(
            InstallPlan::build(options),
            Err(Phase4Error::ElevationRequired { .. })
        ));
    }

    #[test]
    fn current_user_autostart_rejects_editor_mode_arguments() {
        let options = InstallOptions::current_user(Path::new(
            r"C:\Program Files\RED SAMURAI\redsamurai-config.exe",
        ))
        .with_launch_arguments(["--editor"]);

        let error = InstallPlan::build(options)
            .expect_err("current-user startup must always launch the resident tray");

        assert!(matches!(
            error,
            Phase4Error::InvalidArgument {
                reason: "current-user autostart requires exactly --tray",
                ..
            }
        ));
    }

    #[test]
    fn install_plan_rejects_an_autostart_executable_outside_install_directory() {
        let options = InstallOptions::current_user(Path::new(r"C:\Build\redsamurai-config.exe"))
            .with_install_directory(r"C:\Users\tester\AppData\Local\RED SAMURAI")
            .with_data_directory(r"C:\Users\tester\Documents\RED SAMURAI");

        let error = InstallPlan::build(options)
            .expect_err("autostart must point at the executable in the install directory");

        assert!(matches!(error, Phase4Error::InvalidExecutablePath { .. }));
    }
}
