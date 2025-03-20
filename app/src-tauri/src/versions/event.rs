use serde::Serialize;

pub const VERSION_CHECK_STATUS_EVENT: &str = "version-check-status";

#[derive(Serialize, Clone)]
pub enum CheckStatus {
  Manifest,
  Assets,
  Java,
}
