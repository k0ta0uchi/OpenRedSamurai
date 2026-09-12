//! Library root. Module ownership:
//! - `ini`      : coordinator (done)
//! - `profile`  : Worker 1 (CONTRACT_profile.md)
//! - `funcs`    : Worker 2 (CONTRACT_funcs.md)
//! - `assets` / `ui_common` : Worker 3 (CONTRACT_ui.md)
//! - `app`, `ui_frame`, `ui_general`, `ui_dpi`, `ui_light`, `ui_info` : Worker 4 (CONTRACT_app.md)
//!
//! Phase 2 keeps the hardware boundary explicit: `device` owns discovery and
//! feature-report I/O, `device_protocol` owns the wire schema, and
//! `device_apply` owns the fail-closed profile-to-device plan.  Registering
//! these modules here makes their public seams available to the app without
//! making startup perform any device I/O.

pub mod app;
pub mod assets;
pub mod device;
pub mod device_apply;
pub mod device_protocol;
pub mod device_runtime;
pub mod funcs;
pub mod ini;
pub mod installer;
pub mod keyboard_relay;
pub mod keyboard_relay_windows;
pub mod keyboard_suppression;
pub mod keycapture;
pub mod macro_db;
pub mod menu;
pub mod microphone;
pub mod phase4;
pub mod profile;
pub mod resident;
pub mod resident_platform;
pub mod resident_service;
pub mod tray;
pub mod ui_common;
pub mod ui_dialog;
pub mod ui_dpi;
pub mod ui_frame;
pub mod ui_general;
pub mod ui_info;
pub mod ui_light;
pub mod ui_macro;
