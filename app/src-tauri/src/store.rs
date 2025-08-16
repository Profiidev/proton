use std::sync::Arc;

use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use tauri::{AppHandle, Wry};
use tauri_plugin_store::{Store, StoreExt};

const STORE_FILE: &str = "store.json";

pub struct AppStore {
  pub store: Arc<Store<Wry>>,
}

pub trait TauriAppStoreExt {
  fn app_store(&self) -> Result<AppStore>;
}

impl TauriAppStoreExt for AppHandle {
  fn app_store(&self) -> Result<AppStore> {
    let store = self.store(STORE_FILE)?;

    Ok(AppStore { store })
  }
}

impl AppStore {
  pub fn set<V: Serialize>(&self, key: &str, value: &V) -> Result<()> {
    let json = serde_json::to_value(value)?;
    self.store.set(key, json);

    Ok(())
  }

  pub fn get_or_default<V: DeserializeOwned + Default>(&self, key: &str) -> Result<V> {
    let Some(json) = self.store.get(key) else {
      return Ok(V::default());
    };

    Ok(serde_json::from_value(json)?)
  }
}
