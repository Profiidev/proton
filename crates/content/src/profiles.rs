//! Profile lookups through the profiles service, copied out of the store lock so it is never held
//! across network calls.

use std::path::PathBuf;

use anyhow::Result;
use proton_profiles::{ProfilesHandle, config::Profile};

/// A profile, its directory and whether an instance of it runs
pub struct Target {
  pub profile: Profile,
  pub dir: PathBuf,
  pub running: bool,
}

pub async fn resolve(profiles: &ProfilesHandle, id: &str) -> Result<Target> {
  let store = profiles.store.lock().await;
  Ok(Target {
    profile: store.profile(id).await?,
    dir: store.get_profile_path(id)?,
    running: store.is_running(id).await,
  })
}

/// Every profile with its directory
pub async fn all(profiles: &ProfilesHandle) -> Result<Vec<(Profile, PathBuf)>> {
  let store = profiles.store.lock().await;
  Ok(
    store
      .list_profiles()
      .await?
      .into_iter()
      .filter_map(|p| Some((store.get_profile_path(&p.id).ok()?, p)))
      .map(|(dir, p)| (p, dir))
      .collect(),
  )
}
