use thiserror::Error;

pub mod curseforge;
pub mod http;
pub mod install;
pub mod model;
pub mod modpack;
pub mod modrinth;
pub mod provider;
pub mod render;
pub mod store;
pub mod updates;

pub const CF_KEY: &str = match option_env!("CURSEFORGE_API_KEY") {
  Some(k) => k,
  None => "$2a$10$LcjEjkIPGcXllU7ufwNLX.tSj3/Zp6h/aCJmnBNJueYp/sPKwo4OW",
};
pub const USER_AGENT: &str = concat!(
  "ProfiiDev/proton/",
  env!("CARGO_PKG_VERSION"),
  " (github.com/ProfiiDev/proton)"
);
pub use proton_profiles::{MANIFEST, TMP_DIR};

#[derive(Error, Debug)]
pub enum ContentError {
  #[error("Incompatible")]
  Incompatible,
  #[error("NeedsManualDownload")]
  NeedsManualDownload,
  #[error("Network")]
  Network,
  #[error("RateLimited")]
  RateLimited,
  #[error("HashMismatch")]
  HashMismatch,
  #[error("Unsupported")]
  Unsupported,
  #[error("Running")]
  Running,
  #[error("NotFound")]
  NotFound,
  #[error("InvalidPack")]
  InvalidPack,
}

impl From<ContentError> for tauri::Error {
  fn from(err: ContentError) -> Self {
    tauri::Error::Anyhow(err.into())
  }
}

pub type Result<T> = std::result::Result<T, ContentError>;
