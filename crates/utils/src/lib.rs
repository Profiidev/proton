//! Helpers shared by every crate: app dirs, the tokio runtime ([`rt`]), logging, downloads,
//! `write_changed`. No gpui state.

pub mod dir;
pub mod download;
pub mod entity;
pub mod file;
pub mod future;
pub mod http;
pub mod log;
pub mod logs;
pub mod macros;
pub mod rt;
#[cfg(any(test, feature = "test-support"))]
pub mod test_http;

pub const CLIENT_ID: &str = "dd35660a-6381-41f8-bb34-2a36669581d0";
