//! Key-value JSON store, a drop-in for the old tauri-plugin-store file: same path
//! (`<data_dir>/store.json`) and same format (pretty JSON, one top-level object).

use std::{
  path::{Path, PathBuf},
  sync::{Mutex, MutexGuard, OnceLock},
};

use anyhow::Result;
use proton_utils::{dir, log::ErrorLogExt};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

const STORE_FILE: &str = "store.json";

static STORE: OnceLock<Store> = OnceLock::new();

/// The app store, loaded on first use
pub fn store() -> &'static Store {
  STORE.get_or_init(|| {
    let path = dir::data_dir()
      .map(|dir| dir.join(STORE_FILE))
      .expect("No data dir");
    Store::open(path)
  })
}

/// Points [`store`] at another file, for tests of crates that save through it. Only works
/// before the first [`store`] call, later calls are ignored.
pub fn init_at(path: PathBuf) {
  if STORE.set(Store::open(path)).is_err() {
    tracing::warn!("Store already initialized");
  }
}

pub struct Store {
  path: PathBuf,
  data: Mutex<Map<String, Value>>,
}

impl Store {
  /// A missing file is an empty store. An unreadable one is copied to `store.json.corrupt`
  /// before the next save overwrites it.
  pub fn open(path: PathBuf) -> Self {
    let data = match std::fs::read(&path) {
      Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        tracing::error!("Failed to parse {}: {e}", path.display());
        let _ = std::fs::copy(&path, path.with_extension("json.corrupt")).log_err();
        Map::new()
      }),
      Err(_) => Map::new(),
    };
    Self {
      path,
      data: Mutex::new(data),
    }
  }

  fn data(&self) -> MutexGuard<'_, Map<String, Value>> {
    self.data.lock().unwrap_or_else(|e| e.into_inner())
  }

  pub fn get<V: DeserializeOwned>(&self, key: &str) -> Result<Option<V>> {
    let Some(json) = self.data().get(key).cloned() else {
      return Ok(None);
    };
    Ok(Some(serde_json::from_value(json)?))
  }

  pub fn get_or_default<V: DeserializeOwned + Default>(&self, key: &str) -> Result<V> {
    Ok(self.get(key)?.unwrap_or_default())
  }

  /// Sets and saves right away
  pub fn set<V: Serialize>(&self, key: &str, value: &V) -> Result<()> {
    let json = serde_json::to_value(value)?;
    let mut data = self.data();
    let old = data.insert(key.to_string(), json);
    let res = save(&self.path, &data);
    // a failed save must not leak into the next successful one
    if res.is_err() {
      match old {
        Some(old) => data.insert(key.to_string(), old),
        None => data.remove(key),
      };
    }
    res
  }

  pub fn delete(&self, key: &str) -> Result<()> {
    let mut data = self.data();
    let Some(old) = data.remove(key) else {
      return Ok(());
    };
    let res = save(&self.path, &data);
    if res.is_err() {
      data.insert(key.to_string(), old);
    }
    res
  }

  pub fn save(&self) -> Result<()> {
    save(&self.path, &self.data())
  }
}

/// Writes a temp file next to the store and renames it, a crash never leaves half a file
fn save(path: &Path, data: &Map<String, Value>) -> Result<()> {
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  let tmp = path.with_extension("json.tmp");
  std::fs::write(&tmp, serde_json::to_vec_pretty(data)?)?;
  std::fs::rename(&tmp, path)?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use serde::Deserialize;

  use super::*;

  const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/io.profidev.proton/store.json"
  );

  #[derive(Serialize, Deserialize, Default, PartialEq, Debug)]
  struct Item {
    name: String,
    count: u32,
  }

  #[test]
  fn round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join(STORE_FILE);
    let store = Store::open(path.clone());
    assert_eq!(store.get::<Item>("item").unwrap(), None);
    assert_eq!(
      store.get_or_default::<Item>("item").unwrap(),
      Item::default()
    );

    let item = Item {
      name: "a".into(),
      count: 2,
    };
    store.set("item", &item).unwrap();
    store.set("other", &1).unwrap();
    store.delete("other").unwrap();

    let reopened = Store::open(path.clone());
    assert_eq!(reopened.get::<Item>("item").unwrap(), Some(item));
    assert_eq!(reopened.get::<u32>("other").unwrap(), None);
    // pretty printed like tauri-plugin-store, no temp file left
    assert!(
      std::fs::read_to_string(&path)
        .unwrap()
        .starts_with("{\n  \"")
    );
    assert!(!path.with_extension("json.tmp").exists());
  }

  #[test]
  fn reads_the_tauri_store() {
    let store = Store::open(FIXTURE.into());
    let accounts: HashMap<String, Value> = store.get_or_default("accounts").unwrap();
    assert!(accounts.contains_key("f1f1f1f1f1f14f1f8f1f1f1f1f1f1f1f"));

    // saving keeps every key and value
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join(STORE_FILE);
    std::fs::copy(FIXTURE, &copy).unwrap();
    Store::open(copy.clone()).save().unwrap();
    let read = |p: &str| serde_json::from_slice::<Value>(&std::fs::read(p).unwrap()).unwrap();
    assert_eq!(read(copy.to_str().unwrap()), read(FIXTURE));
  }

  #[test]
  fn corrupt_file_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(STORE_FILE);
    std::fs::write(&path, "{nope").unwrap();
    let store = Store::open(path.clone());
    store.set("a", &1).unwrap();
    assert_eq!(
      std::fs::read_to_string(path.with_extension("json.corrupt")).unwrap(),
      "{nope"
    );
  }

  #[test]
  fn non_object_json_counts_as_corrupt() {
    // valid json but not one top-level object
    for body in ["[1,2]", "\"text\"", "null", ""] {
      let dir = tempfile::tempdir().unwrap();
      let path = dir.path().join(STORE_FILE);
      std::fs::write(&path, body).unwrap();
      let store = Store::open(path.clone());
      assert_eq!(store.get::<u32>("a").unwrap(), None, "{body:?}");
      assert_eq!(
        std::fs::read_to_string(path.with_extension("json.corrupt")).unwrap(),
        body
      );
    }
  }

  #[test]
  fn wrong_type_is_an_error_not_a_default() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join(STORE_FILE));
    store.set("item", &"a string").unwrap();
    assert!(store.get::<Item>("item").is_err());
    assert!(store.get_or_default::<Item>("item").is_err());
    // missing fields are an error too, callers add #[serde(default)]
    store
      .set("item", &serde_json::json!({ "name": "a" }))
      .unwrap();
    assert!(store.get::<Item>("item").is_err());
  }

  #[test]
  fn deleting_a_missing_key_does_not_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(STORE_FILE);
    let store = Store::open(path.clone());
    store.delete("nope").unwrap();
    assert!(!path.exists());
  }

  #[test]
  fn set_overwrites_and_keeps_unicode_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(STORE_FILE);
    let store = Store::open(path.clone());
    store.set("schlüssel ✓", &1).unwrap();
    store.set("schlüssel ✓", &2).unwrap();
    assert_eq!(
      Store::open(path).get::<u32>("schlüssel ✓").unwrap(),
      Some(2)
    );
  }

  #[test]
  fn unwritable_path_fails_the_save() {
    let dir = tempfile::tempdir().unwrap();
    // the parent is a file, so create_dir_all fails
    let blocker = dir.path().join("file");
    std::fs::write(&blocker, "").unwrap();
    let store = Store::open(blocker.join(STORE_FILE));
    assert!(store.set("a", &1).is_err());
    // a failed save leaves nothing behind that a later save would write
    assert_eq!(store.get::<u32>("a").unwrap(), None);
  }

  #[test]
  fn failed_saves_restore_the_old_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(STORE_FILE);
    let store = Store::open(path.clone());
    store.set("a", &1).unwrap();
    store.set("b", &2).unwrap();
    // the temp file can't be created next to the store anymore
    std::fs::create_dir(path.with_extension("json.tmp")).unwrap();
    assert!(store.set("a", &9).is_err());
    assert!(store.delete("b").is_err());
    assert_eq!(store.get::<u32>("a").unwrap(), Some(1));
    assert_eq!(store.get::<u32>("b").unwrap(), Some(2));
  }

  #[test]
  fn init_at_wins_once_and_later_calls_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.json");
    let second = dir.path().join("second.json");
    init_at(first.clone());
    init_at(second.clone());
    store().set("a", &1).unwrap();
    assert!(first.exists());
    assert!(!second.exists());
  }
}
