use std::{
  io::Read,
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Serialize, de::DeserializeOwned};
use sha1::{Digest, Sha1};
use thiserror::Error;
use tokio::fs::{self, File};

use crate::rt::spawn_blocking;

#[derive(Error, Debug)]
pub enum FileError {
  #[error("HashMismatch")]
  HashMismatch,
}

pub async fn file_hash(hash: &str, path: &PathBuf) -> Result<bool> {
  let Ok(file) = File::open(path).await else {
    return Ok(false);
  };
  let mut file = file.into_std().await;
  let found_hash = spawn_blocking(move || {
    let mut hasher = Sha1::new();
    let mut buffer = [0; 8192];
    loop {
      let bytes_read = file.read(&mut buffer)?;
      if bytes_read == 0 {
        break;
      }
      hasher.update(&buffer[..bytes_read]);
    }
    Ok::<_, std::io::Error>(hex::encode(hasher.finalize()))
  })
  .await??;
  Ok(hash == found_hash)
}

/// Hex sha1 of `bytes`
pub fn bytes_hash(bytes: &[u8]) -> String {
  hex::encode(Sha1::digest(bytes))
}

pub async fn read_parse_file<R: DeserializeOwned>(path: &PathBuf) -> Result<R> {
  let data = fs::read_to_string(path).await?;
  Ok(serde_json::from_str(&data)?)
}

pub async fn read_parse_xml_file<R: DeserializeOwned>(path: &PathBuf) -> Result<R> {
  let data = fs::read_to_string(path).await?;
  Ok(serde_xml_rs::from_str(&data)?)
}

/// Writes JSON through a temp file and a rename, so a concurrent reader never sees a cut file
pub async fn write_file<T: Serialize>(path: &PathBuf, data: &T) -> Result<()> {
  static NEXT: AtomicU64 = AtomicU64::new(0);
  let data = serde_json::to_string(data)?;
  // unique per write, two writers never share a temp file
  let n = NEXT.fetch_add(1, Ordering::Relaxed);
  let mut tmp = path.clone().into_os_string();
  tmp.push(format!(".{}.{n}.tmp", std::process::id()));
  fs::write(&tmp, data).await?;
  if let Err(e) = fs::rename(&tmp, path).await {
    let _ = fs::remove_file(&tmp).await;
    return Err(e.into());
  }
  Ok(())
}

pub async fn create_or_open_file(path: &PathBuf) -> Result<File> {
  let path = Path::new(path);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).await?;
  }
  Ok(File::create(path).await?)
}

pub async fn last_modified_ago(path: &PathBuf) -> Result<Option<Duration>> {
  if !path.exists() {
    return Ok(None);
  }
  let metadata = fs::metadata(path).await?;
  let sys_time = metadata.modified()?;
  let time: DateTime<Utc> = sys_time.into();

  Ok(Some(Utc::now() - time))
}

#[cfg(test)]
mod tests {
  use serde::Deserialize;

  use super::*;
  use crate::rt::block_on;

  #[derive(Serialize, Deserialize, Debug, PartialEq)]
  struct Item {
    name: String,
    count: u32,
  }

  #[test]
  fn bytes_hash_is_hex_sha1() {
    let cases: [(&[u8], &str); 3] = [
      (b"", "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
      (b"abc", "a9993e364706816aba3e25717850c26c9cd0d89d"),
      ("ü".as_bytes(), "94a759fd37735430753c7b6b80684306d80ea16e"),
    ];
    for (bytes, hash) in cases {
      assert_eq!(bytes_hash(bytes), hash, "{bytes:?}");
    }
  }

  #[test]
  fn file_hash_compares_the_file_content() {
    let dir = tempfile::tempdir().unwrap();
    // spans several 8 KiB reads with a partial last one
    let big = (0..20_000u32).map(|i| i as u8).collect::<Vec<_>>();
    let cases: [(&str, &[u8]); 3] = [("empty", b""), ("small", b"abc"), ("big", &big)];
    for (name, bytes) in cases {
      let path = dir.path().join(name);
      std::fs::write(&path, bytes).unwrap();
      assert!(
        block_on(file_hash(&bytes_hash(bytes), &path)).unwrap(),
        "{name}"
      );
      assert!(
        !block_on(file_hash(&bytes_hash(b"other"), &path)).unwrap(),
        "{name}"
      );
    }
    // a missing file is a mismatch, not an error
    assert!(!block_on(file_hash(&bytes_hash(b""), &dir.path().join("missing"))).unwrap());
  }

  #[test]
  fn file_hash_is_case_sensitive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    std::fs::write(&path, "abc").unwrap();
    // hex::encode is lowercase, an uppercase hash never matches
    let upper = bytes_hash(b"abc").to_uppercase();
    assert!(!block_on(file_hash(&upper, &path)).unwrap());
  }

  #[test]
  fn write_then_read_json_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("item.json");
    let item = Item {
      name: "名前 ✓".into(),
      count: 1,
    };
    block_on(write_file(&path, &item)).unwrap();
    assert_eq!(block_on(read_parse_file::<Item>(&path)).unwrap(), item);
  }

  #[test]
  fn write_file_does_not_create_parents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing/item.json");
    assert!(block_on(write_file(&path, &1)).is_err());
  }

  #[test]
  fn concurrent_readers_never_see_a_cut_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("item.json");
    let big = "x".repeat(1 << 20);
    block_on(write_file(&path, &big)).unwrap();
    let writer = {
      let path = path.clone();
      let big = big.clone();
      std::thread::spawn(move || {
        for _ in 0..50 {
          block_on(write_file(&path, &big)).unwrap();
        }
      })
    };
    while !writer.is_finished() {
      // a plain truncate-then-write shows an empty or short file here
      let read: String = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
      assert_eq!(read.len(), big.len());
    }
    writer.join().unwrap();
    // no temp files left behind
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
  }

  #[test]
  fn read_parse_file_errors() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
      ("empty", ""),
      ("malformed", "{\"name\":"),
      ("wrong_type", r#"{"name":"a","count":"x"}"#),
      ("missing_field", r#"{"name":"a"}"#),
    ];
    for (name, body) in cases {
      let path = dir.path().join(name);
      std::fs::write(&path, body).unwrap();
      let err = block_on(read_parse_file::<Item>(&path)).unwrap_err();
      assert!(err.is::<serde_json::Error>(), "{name}");
    }
    let err = block_on(read_parse_file::<Item>(&dir.path().join("missing"))).unwrap_err();
    assert!(err.is::<std::io::Error>());
  }

  #[test]
  fn read_parse_xml_file_reads_and_rejects_garbage() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ok.xml");
    std::fs::write(&path, "<item><name>ünï</name><count>4</count></item>").unwrap();
    assert_eq!(
      block_on(read_parse_xml_file::<Item>(&path)).unwrap(),
      Item {
        name: "ünï".into(),
        count: 4
      }
    );

    for (name, body) in [("unclosed", "<item><name>a</name>"), ("empty", "")] {
      let path = dir.path().join(name);
      std::fs::write(&path, body).unwrap();
      assert!(
        block_on(read_parse_xml_file::<Item>(&path)).is_err(),
        "{name}"
      );
    }
    assert!(block_on(read_parse_xml_file::<Item>(&dir.path().join("missing"))).is_err());
  }

  #[test]
  fn create_or_open_file_creates_parents_and_truncates() {
    use tokio::io::AsyncWriteExt;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a/b/log.txt");
    block_on(async {
      let mut file = create_or_open_file(&path).await.unwrap();
      file.write_all(b"first run").await.unwrap();
      file.flush().await.unwrap();
    });
    assert_eq!(std::fs::read(&path).unwrap(), b"first run");

    block_on(async {
      let mut file = create_or_open_file(&path).await.unwrap();
      file.write_all(b"2nd").await.unwrap();
      file.flush().await.unwrap();
    });
    assert_eq!(std::fs::read(&path).unwrap(), b"2nd");
  }

  #[test]
  fn last_modified_ago_is_none_for_missing_files() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
      block_on(last_modified_ago(&dir.path().join("missing"))).unwrap(),
      None
    );

    let path = dir.path().join("f");
    std::fs::write(&path, "").unwrap();
    let ago = block_on(last_modified_ago(&path)).unwrap().unwrap();
    assert!(ago < Duration::minutes(1), "{ago}");

    // an old mtime shows up as a long duration
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 3600);
    std::fs::File::options()
      .write(true)
      .open(&path)
      .unwrap()
      .set_modified(old)
      .unwrap();
    let ago = block_on(last_modified_ago(&path)).unwrap().unwrap();
    assert!(
      ago >= Duration::hours(3) && ago < Duration::hours(4),
      "{ago}"
    );
  }
}
