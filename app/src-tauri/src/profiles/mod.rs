pub mod commands;
pub mod config;
mod instance;
mod profile;
pub mod store;
mod watcher;

const PROFILE_DIR: &str = "profiles";
const PROFILE_CONFIG: &str = "profile.json";
const PROFILE_IMAGE: &str = "image.png";
const PROFILE_LOGS: &str = "instance_logs";
const SAVES_DIR: &str = "saves";
