pub mod config;
pub mod instance;
pub mod profile;
pub mod store;
pub mod watcher;

const PROFILE_DIR: &str = "profiles";
const PROFILE_CONFIG: &str = "profile.json";
const PROFILE_IMAGE: &str = "image.png";
const PROFILE_LOGS: &str = "instance_logs";
const SAVES_DIR: &str = "saves";
/// Content manifest kept in each content folder
pub const MANIFEST: &str = "proton-content.json";
/// Downloads land here before they are verified and moved into place
pub const TMP_DIR: &str = ".proton-tmp";
