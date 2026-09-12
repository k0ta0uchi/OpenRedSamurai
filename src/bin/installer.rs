#![windows_subsystem = "windows"]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use redsamurai_config::installer::{
    self, AppVersion, InstallOutcome, ReleaseInfo, UpdateError, CURRENT_VERSION,
    INSTALLER_EXECUTABLE_NAME,
};

const WINDOW_SIZE: [f32; 2] = [560.0, 430.0];

enum TaskMessage {
    Checked(Result<ReleaseInfo, UpdateError>),
    Installed(Result<InstallOutcome, UpdateError>),
    Updated(Result<UpdateInstallResult, UpdateError>),
    Uninstalled(Result<(), UpdateError>),
}

enum UpdateInstallResult {
    Completed {
        release: ReleaseInfo,
        outcome: InstallOutcome,
    },
    Deferred {
        release: ReleaseInfo,
    },
}

struct InstallerApp {
    install_directory: PathBuf,
    data_directory: PathBuf,
    latest: Option<ReleaseInfo>,
    status: String,
    busy: bool,
    auto_check: bool,
    uninstall_mode: bool,
    receiver: Option<Receiver<TaskMessage>>,
    close_after_update: bool,
}

impl InstallerApp {
    fn new(auto_check: bool, uninstall_mode: bool) -> Self {
        let install_directory = installer::default_install_directory().unwrap_or_else(|_| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("RED SAMURAI")
        });
        let data_directory = installer::default_data_directory()
            .unwrap_or_else(|_| install_directory_fallback_data(&install_directory));
        let status = if uninstall_mode {
            "アンインストールを準備しています".to_owned()
        } else if auto_check {
            "GitHub Releasesで最新版を確認します".to_owned()
        } else {
            "インストールの準備ができました".to_owned()
        };
        Self {
            install_directory,
            data_directory,
            latest: None,
            status,
            busy: false,
            auto_check,
            uninstall_mode,
            receiver: None,
            close_after_update: false,
        }
    }

    fn poll_task(&mut self) {
        let Some(receiver) = self.receiver.as_ref() else {
            return;
        };
        let message = match receiver.try_recv() {
            Ok(message) => message,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.receiver = None;
                self.busy = false;
                self.status = "処理スレッドが終了しました".to_owned();
                return;
            }
        };
        self.receiver = None;
        self.busy = false;
        match message {
            TaskMessage::Checked(result) => match result {
                Ok(release) => {
                    let current = AppVersion::current();
                    self.status = if release.version > current {
                        format!("更新があります: v{}（現在 v{}）", release.version, current)
                    } else {
                        format!("最新版です: v{}", current)
                    };
                    self.latest = Some(release);
                }
                Err(error) => self.status = error.to_string(),
            },
            TaskMessage::Installed(result) => match result {
                Ok(outcome) => {
                    self.status = format!(
                        "インストール完了: v{} — {}",
                        outcome.version,
                        outcome.executable_path.display()
                    );
                }
                Err(error) => self.status = error.to_string(),
            },
            TaskMessage::Updated(result) => match result {
                Ok(UpdateInstallResult::Completed { release, outcome }) => {
                    self.latest = Some(release.clone());
                    self.status = format!(
                        "更新完了: v{} — {}",
                        outcome.version,
                        outcome.executable_path.display()
                    );
                }
                Ok(UpdateInstallResult::Deferred { release }) => {
                    self.latest = Some(release);
                    self.status =
                        "更新を準備しました。実行中のアプリを閉じた後に適用します".to_owned();
                    self.close_after_update = true;
                }
                Err(error) => self.status = error.to_string(),
            },
            TaskMessage::Uninstalled(result) => match result {
                Ok(()) => self.status = "アンインストール完了（データは保持しました）".to_owned(),
                Err(error) => self.status = error.to_string(),
            },
        }
    }

    fn start_check(&mut self) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "GitHub Releasesを確認中…".to_owned();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(TaskMessage::Checked(installer::fetch_latest_release()));
        });
        self.receiver = Some(receiver);
    }

    fn start_install(&mut self) {
        if self.busy {
            return;
        }
        let source_directory = match std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
        {
            Some(path) => path,
            None => {
                self.status = "インストーラーの配置場所を取得できません".to_owned();
                return;
            }
        };
        self.busy = true;
        self.status = "ファイルを配置し、自動起動を登録中…".to_owned();
        let install_directory = self.install_directory.clone();
        let data_directory = self.data_directory.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result =
                installer::install_payload(&source_directory, &install_directory, &data_directory);
            let _ = sender.send(TaskMessage::Installed(result));
        });
        self.receiver = Some(receiver);
    }

    fn start_update(&mut self) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "最新版をダウンロードし、検証してから更新中…".to_owned();
        let install_directory = self.install_directory.clone();
        let data_directory = self.data_directory.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = update_and_install(&install_directory, &data_directory);
            let _ = sender.send(TaskMessage::Updated(result));
        });
        self.receiver = Some(receiver);
    }

    fn start_uninstall(&mut self) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "自動起動とインストール済みファイルを削除中…".to_owned();
        let install_directory = self.install_directory.clone();
        let data_directory = self.data_directory.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = installer::uninstall_current_user(&install_directory, &data_directory);
            let _ = sender.send(TaskMessage::Uninstalled(result));
        });
        self.receiver = Some(receiver);
    }
}

impl eframe::App for InstallerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_task();
        if self.close_after_update {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.auto_check {
            self.auto_check = false;
            self.start_check();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(8, 9, 10)))
            .show(ctx, |ui| {
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(245, 36, 42), "■");
                    ui.heading("RED SAMURAI インストーラー");
                });
                ui.label(
                    egui::RichText::new("現在のユーザーだけに安全にインストール・更新します")
                        .color(egui::Color32::from_rgb(160, 164, 174)),
                );
                ui.add_space(18.0);

                ui.group(|ui| {
                    ui.label(format!("現在のバージョン: v{CURRENT_VERSION}"));
                    ui.label(format!(
                        "インストール先: {}",
                        self.install_directory.display()
                    ));
                    ui.label(format!("データ先: {}", self.data_directory.display()));
                });
                ui.add_space(14.0);

                let check = ui.add_enabled(!self.busy, egui::Button::new("最新版を確認"));
                if check.clicked() {
                    self.start_check();
                }
                if let Some(release) = self.latest.as_ref() {
                    ui.label(format!("GitHub最新: v{}", release.version));
                    let can_update = release.version > AppVersion::current();
                    let update = ui.add_enabled(
                        !self.busy && can_update,
                        egui::Button::new("ダウンロードして更新"),
                    );
                    if update.clicked() {
                        self.start_update();
                    }
                }
                ui.add_space(12.0);
                if self.uninstall_mode {
                    let uninstall =
                        ui.add_enabled(!self.busy, egui::Button::new("アンインストール"));
                    if uninstall.clicked() {
                        self.start_uninstall();
                    }
                } else {
                    let install = ui.add_enabled(!self.busy, egui::Button::new("インストール"));
                    if install.clicked() {
                        self.start_install();
                    }
                }
                ui.separator();
                ui.label(
                    egui::RichText::new(&self.status).color(egui::Color32::from_rgb(220, 224, 232)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("更新元: GitHub Releases / SHA-256 sidecar検証")
                        .small()
                        .color(egui::Color32::from_rgb(125, 130, 140)),
                );
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn install_directory_fallback_data(install_directory: &Path) -> PathBuf {
    install_directory
        .parent()
        .unwrap_or(install_directory)
        .join("OpenRedSamurai")
        .join(installer::PRODUCT_NAME)
}

fn update_and_install(
    install_directory: &Path,
    data_directory: &Path,
) -> Result<UpdateInstallResult, UpdateError> {
    let release = installer::fetch_latest_release()?;
    if release.version <= AppVersion::current() {
        return Err(UpdateError::InvalidRelease(format!(
            "最新版 v{} は現在の v{} 以下です",
            release.version,
            AppVersion::current()
        )));
    }
    let temp_root = std::env::temp_dir()
        .join("OpenRedSamurai")
        .join(format!("update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&temp_root)?;
    let package = temp_root.join(&release.package_name);
    let result = (|| {
        installer::download_verified_release(&release, &package)?;
        let staging = temp_root.join("stage");
        installer::extract_package(&package, &staging)?;
        let payload = installer::locate_payload_root(&staging)?;
        let current_executable = std::env::current_exe()?;
        let installed_installer = install_directory.join(INSTALLER_EXECUTABLE_NAME);
        if same_file(&current_executable, &installed_installer) {
            // Windows keeps the running installer image open.  Copy this
            // executable to staging and let it apply the payload after this
            // GUI exits; the helper retries while the editor/tray is closed.
            let helper = temp_root.join("OpenRedSamurai-Setup-helper.exe");
            std::fs::copy(&current_executable, &helper)?;
            std::process::Command::new(&helper)
                .arg("--apply-staged")
                .arg(&payload)
                .arg(install_directory)
                .arg(data_directory)
                .arg(release.version.to_string())
                .spawn()
                .map_err(UpdateError::from)?;
            return Ok(UpdateInstallResult::Deferred {
                release: release.clone(),
            });
        }
        installer::install_payload_with_version(
            &payload,
            install_directory,
            data_directory,
            release.version,
        )
        .map(|outcome| UpdateInstallResult::Completed {
            release: release.clone(),
            outcome,
        })
    })();
    if !matches!(result, Ok(UpdateInstallResult::Deferred { .. })) {
        let _ = std::fs::remove_dir_all(&temp_root);
    }
    result
}

fn same_file(left: &Path, right: &Path) -> bool {
    let left = std::fs::canonicalize(left).ok();
    let right = std::fs::canonicalize(right).ok();
    left.is_some() && left == right
}

fn apply_staged_payload(args: &[String]) -> Result<(), UpdateError> {
    if args.len() != 4 {
        return Err(UpdateError::Io(
            "--apply-staged requires source, install, data and version".to_owned(),
        ));
    }
    let source = PathBuf::from(&args[0]);
    let install = PathBuf::from(&args[1]);
    let data = PathBuf::from(&args[2]);
    let version = AppVersion::parse(&args[3])?;
    let mut last_error = None;
    for _ in 0..180 {
        match installer::install_payload_with_version(&source, &install, &data, version) {
            Ok(_) => return Ok(()),
            Err(UpdateError::Io(message)) => {
                last_error = Some(message);
                thread::sleep(Duration::from_millis(500));
            }
            Err(error) => return Err(error),
        }
    }
    Err(UpdateError::Io(format!(
        "更新対象が使用中のため適用できません: {}",
        last_error.unwrap_or_else(|| "タイムアウト".to_owned())
    )))
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args
        .first()
        .is_some_and(|arg| arg.eq_ignore_ascii_case("--apply-staged"))
    {
        if let Err(error) = apply_staged_payload(&args[1..]) {
            eprintln!("OpenRedSamurai updater helper: {error}");
            std::process::exit(1);
        }
        return;
    }
    let auto_check = args.iter().any(|arg| arg.eq_ignore_ascii_case("--update"));
    let uninstall_mode = args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("--uninstall"));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(WINDOW_SIZE[0], WINDOW_SIZE[1]))
            .with_min_inner_size(egui::vec2(WINDOW_SIZE[0], WINDOW_SIZE[1]))
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../../assets/icons/redsamurai.png"
                ))
                .expect("embedded RED SAMURAI icon must be valid PNG data"),
            ),
        ..Default::default()
    };
    let result = eframe::run_native(
        INSTALLER_EXECUTABLE_NAME,
        options,
        Box::new(move |cc| {
            load_japanese_font(&cc.egui_ctx);
            Ok(Box::new(InstallerApp::new(auto_check, uninstall_mode)))
        }),
    );
    if let Err(error) = result {
        eprintln!("OpenRedSamurai installer: {error}");
        std::process::exit(1);
    }
}

fn load_japanese_font(ctx: &egui::Context) {
    const CANDIDATES: [&str; 5] = [
        "meiryo.ttc",
        "YuGothM.ttc",
        "YuGothR.ttc",
        "msgothic.ttc",
        "msyh.ttc",
    ];
    for name in CANDIDATES {
        let path = std::path::Path::new("C:\\Windows\\Fonts").join(name);
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("jp".to_owned(), egui::FontData::from_owned(bytes));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "jp".to_owned());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("jp".to_owned());
            ctx.set_fonts(fonts);
            return;
        }
    }
}
