//! Helpers shared by every crate: app dirs, the tokio runtime ([`rt`]), logging, downloads,
//! `write_changed`. No gpui state.

pub mod dir;
pub mod download;
pub mod entity;
pub mod file;
pub mod future;
pub mod log;
pub mod logs;
pub mod macros;
pub mod rt;
pub mod tauri_compat;

pub const CLIENT_ID: &str = "dd35660a-6381-41f8-bb34-2a36669581d0";
