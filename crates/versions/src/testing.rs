//! Test helpers: zips and a fake java (also for other crates via `test-support`), plus a temp
//! data dir and a served version for this crate's tests.

use std::{
  os::unix::fs::PermissionsExt,
  path::{Path, PathBuf},
};

use async_zip::{Compression, ZipEntryBuilder, base::write::ZipFileWriter};
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[cfg(test)]
/// A fresh data dir, `proton_utils::dir::data_dir()` resolves into it for this process
pub fn data_dir() -> PathBuf {
  let base = tempfile::tempdir().unwrap().keep();
  // nextest runs one process per test, so the env var is not shared
  unsafe { std::env::set_var("XDG_DATA_HOME", &base) };
  proton_utils::dir::data_dir().unwrap()
}

pub async fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
  tokio::fs::create_dir_all(path.parent().unwrap())
    .await
    .unwrap();
  let out = tokio::fs::File::create(path).await.unwrap();
  let mut writer = ZipFileWriter::new(out.compat_write());
  for (name, body) in entries {
    let entry = ZipEntryBuilder::new((*name).into(), Compression::Stored);
    writer.write_entry_whole(entry, body).await.unwrap();
  }
  // close leaves the last tokio write in flight, flush before anyone reads the file
  let mut file = writer.close().await.unwrap().into_inner();
  tokio::io::AsyncWriteExt::flush(&mut file).await.unwrap();
}

/// Writes a `sh` java at `bin` that appends its cwd, `$PROTON_TEST_ENV` and args (one per line,
/// then `--end`) to the returned file and exits with `code`
pub fn fake_java(bin: &Path, code: i32) -> PathBuf {
  std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
  let record = bin.with_extension("calls");
  let script = format!(
    "#!/bin/sh\n{{ echo \"cwd=$PWD\"; echo \"env=$PROTON_TEST_ENV\"; for a in \"$@\"; do echo \"$a\"; done; echo --end; }} >> '{}'\nexit {code}\n",
    record.display()
  );
  std::fs::write(bin, script).unwrap();
  std::fs::set_permissions(bin, std::fs::Permissions::from_mode(0o755)).unwrap();
  record
}

/// The recorded invocations of [`fake_java`], each as its lines
pub fn java_calls(record: &Path) -> Vec<Vec<String>> {
  let Ok(text) = std::fs::read_to_string(record) else {
    return Vec::new();
  };
  text
    .split_terminator("--end\n")
    .map(|call| call.lines().map(String::from).collect())
    .collect()
}

#[cfg(test)]
/// A served 1.20.1 with client, two assets, a java runtime, a library, natives and an osx only
/// library that must never be fetched
pub struct VersionFixture {
  pub manifest: crate::meta::minecraft::Manifest,
  pub java: crate::meta::java::JavaVersions,
}

#[cfg(test)]
pub async fn serve_version(server: &proton_utils::test_http::FakeServer) -> VersionFixture {
  use proton_utils::file::bytes_hash;
  use serde_json::json;

  let origin = server.origin();
  let serve = |path: &str, body: &[u8]| {
    server.route("GET", path, 200, body.to_vec());
    json!({ "url": format!("{origin}{path}"), "size": body.len(), "sha1": bytes_hash(body) })
  };

  let mut objects = serde_json::Map::new();
  for (name, body) in [("a.txt", b"asset-one".as_slice()), ("b.txt", b"asset-two")] {
    let hash = bytes_hash(body);
    let path = format!("/resources.download.minecraft.net/{}/{hash}", &hash[..2]);
    server.route("GET", &path, 200, body.to_vec());
    objects.insert(name.into(), json!({ "hash": hash, "size": body.len() }));
  }
  let index = json!({ "objects": objects }).to_string();
  let mut asset_index = serve("/indexes/5.json", index.as_bytes());
  asset_index["id"] = "5".into();
  asset_index["totalSize"] = 18.into();

  let java_bin = serve("/java/bin/java", b"#!/bin/sh\n");
  let java_lib = serve("/java/lib/x.txt", b"x");
  let files = json!({ "files": {
    "bin": { "type": "directory" },
    "bin/java": { "type": "file", "executable": true, "downloads": { "raw": java_bin } },
    "lib/x.txt": { "type": "file", "executable": false, "downloads": { "raw": java_lib } },
    "lib/link": { "type": "link", "target": "x.txt" }
  } })
  .to_string();
  let runtime = json!([{
    "manifest": serve("/java/manifest.json", files.as_bytes()),
    "version": { "name": "17.0.8", "released": "2023-01-01" }
  }]);
  let platform = json!({
    "java-runtime-alpha": [], "java-runtime-beta": [], "java-runtime-delta": [],
    "java-runtime-epsilon": [], "java-runtime-gamma": runtime,
    "java-runtime-gamma-snapshot": [], "jre-legacy": []
  });
  let java = serde_json::from_value(json!({
    "linux": platform, "linux-i386": platform, "mac-os": platform, "mac-os-arm64": platform,
    "windows-arm64": platform, "windows-x64": platform, "windows-x86": platform
  }))
  .unwrap();

  let tmp = tempfile::tempdir().unwrap();
  let natives = tmp.path().join("natives.zip");
  write_zip(
    &natives,
    &[("liblwjgl.so", b"so"), ("META-INF/MANIFEST.MF", b"")],
  )
  .await;
  let mut native = serve("/lib/natives.jar", &std::fs::read(&natives).unwrap());
  native["path"] = "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives.jar".into();
  let mut lib = serve("/lib/lib.jar", b"lib");
  lib["path"] = "a/b/lib/1/lib-1.jar".into();
  let mac = json!({ "url": format!("{origin}/never"), "size": 1, "sha1": "x", "path": "mac.jar" });

  let version = json!({
    "id": "1.20.1",
    "assetIndex": asset_index,
    "downloads": { "client": serve("/client.jar", b"client") },
    "javaVersion": { "component": "java-runtime-gamma", "majorVersion": 17 },
    "libraries": [
      { "name": "a.b:lib:1", "downloads": { "artifact": lib } },
      {
        "name": "org.lwjgl:lwjgl:3.3.1",
        "downloads": { "classifiers": { "natives-linux": native, "natives-osx": mac, "natives-windows": mac } }
      },
      { "name": "a.b:mac:1", "downloads": { "artifact": mac }, "rules": [{ "action": "allow", "os": { "name": "osx" } }] },
      // nothing to download
      { "name": "a.b:bare:1" }
    ],
    "mainClass": "net.minecraft.client.main.Main",
    "type": "release"
  })
  .to_string();
  let mut entry = serve("/v/1.20.1.json", version.as_bytes());
  entry["id"] = "1.20.1".into();
  entry["type"] = "release".into();
  let manifest = serde_json::from_value(json!({ "versions": [entry] })).unwrap();

  VersionFixture { manifest, java }
}

pub fn file_mode(path: &Path) -> u32 {
  std::fs::metadata(path).unwrap().permissions().mode()
}
