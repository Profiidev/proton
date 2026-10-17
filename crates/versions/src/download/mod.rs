use std::{
  future::Future,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use anyhow::Result;
use reqwest::Client;
use thiserror::Error;
use tracing::debug;

use crate::{
  download::{
    assets::check_download_version_assets,
    java::check_download_java_files,
    libraries::check_download_version_java_libraries,
    manifest::{check_assets_manifest, check_client, check_java_manifest, check_version_manifest},
  },
  event::{DownloadCheckStatus, DownloadsHandle},
  loader::LoaderVersion,
  meta::{java::PlatformVersion, minecraft::ManifestVersion},
  paths::{JavaVersionPath, MCPath, MCVersionPath},
};
use proton_utils::future::run_pool;

mod assets;
mod java;
mod libraries;
mod manifest;

#[derive(Error, Debug)]
pub enum DownloadError {
  #[error("Version not found")]
  NotFound,
  #[error("Not supported on this system")]
  NotSupported,
}

pub async fn check_download_version(
  mc: &ManifestVersion,
  java: &PlatformVersion,
  data_dir: &PathBuf,
  client: &Client,
  handle: &DownloadsHandle,
  update_id: usize,
  loader_version: Option<Box<dyn LoaderVersion>>,
) -> Result<()> {
  let mc_path = MCPath::new(data_dir);
  let version_path = MCVersionPath::new(data_dir, &mc.id);

  let version = check_version_manifest(mc, &version_path, client, handle, update_id).await?;
  let java_path = JavaVersionPath::new(data_dir, version.java_version.component, mc.id.clone());
  let assets = check_assets_manifest(&version, &mc_path, client, handle, update_id).await?;
  let files = check_java_manifest(&version, java, &java_path, client, handle, update_id).await?;

  check_client(&version, &version_path, client, handle, update_id).await?;

  check_download_version_assets(&assets, &mc_path, client, handle, update_id).await?;
  check_download_java_files(&files, client, &java_path, handle, update_id).await?;
  let libs = check_download_version_java_libraries(
    &version, client, &java_path, &mc_path, handle, update_id,
  )
  .await?;

  if let Some(loader) = loader_version {
    handle.status(update_id, DownloadCheckStatus::ModLoaderMeta);

    debug!("Collecting checks for mod loader files");
    let futures = loader
      .download(client, &version_path, &mc_path, &libs)
      .await?;
    debug!("Collected {} mod loader file checks", futures.len());

    let futures = check_pool(
      futures,
      handle,
      update_id,
      DownloadCheckStatus::ModLoaderFilesCheck,
    )
    .await?;
    debug!("Completed all checks for mod loader files");

    handle.status(update_id, DownloadCheckStatus::ModLoaderFilesDownloadInfo);
    let mut downloads = Vec::with_capacity(futures.len());
    for fut in futures {
      downloads.push(fut.await?);
    }

    download_pool(
      downloads,
      handle.clone(),
      update_id,
      DownloadCheckStatus::ModLoaderFilesDownload,
    )
    .await?;
    debug!("Completed all downloads for mod loader files");

    debug!("Running mod loader preprocess");
    handle.status(update_id, DownloadCheckStatus::ModLoaderPreprocess);

    loader
      .preprocess(&version_path, &mc_path, java_path.bin_path())
      .await?;

    handle.status(update_id, DownloadCheckStatus::ModLoaderPreprocessDone);
    debug!("Completed mod loader preprocess");
  }

  handle.status(update_id, DownloadCheckStatus::Done);

  Ok(())
}

async fn check_pool<S, F, O>(
  futures: Vec<F>,
  handle: &DownloadsHandle,
  update_id: usize,
  status: S,
) -> Result<Vec<O>>
where
  S: Fn(usize, usize) -> DownloadCheckStatus,
  F: Future<Output = Result<Option<O>>> + Send + 'static,
  O: Send + 'static,
{
  let res = run_pool(futures, |done, total| {
    handle.status(update_id, status(done, total))
  })
  .await;

  let mut futures = Vec::new();
  for result in res {
    if let Some(fut) = result?? {
      futures.push(fut);
    }
  }

  Ok(futures)
}

async fn download_pool<S, F, Fut>(
  funcs: Vec<(F, usize)>,
  handle: DownloadsHandle,
  update_id: usize,
  status: S,
) -> Result<()>
where
  S: Fn(usize, usize) -> DownloadCheckStatus + Clone + Send + 'static,
  F: FnOnce(Box<dyn Fn(usize) + Send + 'static>) -> Fut,
  Fut: Future<Output = Result<()>> + Send + 'static,
{
  let done = Arc::new(AtomicUsize::new(0));
  let total_size: usize = funcs.iter().map(|(_, size)| *size).sum();

  let cb = {
    let status = status.clone();
    let handle = handle.clone();

    Box::new(move |chunk| {
      done.fetch_add(chunk, Ordering::SeqCst);
      let done = done.load(Ordering::SeqCst);
      handle.status(update_id, status(done, total_size))
    })
  };

  let mut futures = Vec::with_capacity(funcs.len());
  for (func, _) in funcs {
    futures.push(func(cb.clone()));
  }

  handle.status(update_id, status(0, total_size));

  for result in run_pool(futures, |_, _| {}).await {
    result??;
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use proton_utils::{download::default_client, rt, test_http::FakeServer};

  use super::*;
  use crate::{
    event::DownloadMsg,
    loader::LoaderType,
    meta::java::Component,
    testing::{file_mode, serve_version},
  };

  /// Waits out the status debounce and returns every status sent
  fn statuses(rx: &flume::Receiver<DownloadMsg>) -> Vec<DownloadCheckStatus> {
    rt::block_on(async { tokio::time::sleep(Duration::from_millis(150)).await });
    rx.try_iter()
      .filter_map(|m| match m {
        DownloadMsg::Status { status, .. } => Some(status),
        _ => None,
      })
      .collect()
  }

  #[cfg(target_os = "linux")]
  #[test]
  fn a_version_downloads_once_and_is_then_only_checked() {
    let server = FakeServer::start();
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("dätä");
    let (handle, rx) = DownloadsHandle::new();
    let client = default_client();

    let fixture = rt::block_on(serve_version(&server));
    let mc = &fixture.manifest.versions[0];
    let java = &fixture.java.linux;
    rt::block_on(check_download_version(
      mc, java, &data, &client, &handle, 1, None,
    ))
    .unwrap();

    let version_path = MCVersionPath::new(&data, "1.20.1");
    let java_path = JavaVersionPath::new(&data, Component::JavaRuntimeGamma, "1.20.1".into());
    let mc_path = MCPath::new(&data);
    assert_eq!(std::fs::read(version_path.client_jar()).unwrap(), b"client");
    assert!(version_path.version_manifest().exists());
    assert!(mc_path.assets_index_path().join("5.json").exists());
    assert_eq!(
      std::fs::read_dir(mc_path.assets_objects_path())
        .unwrap()
        .count(),
      2
    );
    assert_eq!(
      std::fs::read(mc_path.library_path().join("a/b/lib/1/lib-1.jar")).unwrap(),
      b"lib"
    );
    assert_eq!(file_mode(&java_path.bin_path()) & 0o777, 0o755);
    assert_ne!(
      file_mode(&java_path.base_path().join("lib/x.txt")) & 0o111,
      0o111
    );
    assert!(java_path.base_path().join("bin").is_dir());
    // links are left to the runtime, natives are unpacked next to the java libs
    assert!(!java_path.base_path().join("lib/link").exists());
    assert_eq!(
      std::fs::read(java_path.native_path().join("liblwjgl.so")).unwrap(),
      b"so"
    );
    assert!(!java_path.native_path().join("META-INF").exists());

    let seen = statuses(&rx);
    // ticks in between are debounced away, the last one always arrives
    assert_eq!(seen.last(), Some(&DownloadCheckStatus::Done));

    let fetched = server.calls().len();
    // the osx library and natives are never fetched
    assert!(server.calls().iter().all(|c| c.path != "/never"));
    // a native file deleted by the user is unpacked again without a download
    std::fs::remove_file(java_path.native_path().join("liblwjgl.so")).unwrap();
    rt::block_on(check_download_version(
      mc, java, &data, &client, &handle, 2, None,
    ))
    .unwrap();
    assert_eq!(server.calls().len(), fetched, "everything was cached");
    assert!(java_path.native_path().join("liblwjgl.so").exists());

    // a corrupted file is fetched again
    std::fs::write(version_path.client_jar(), "broken").unwrap();
    rt::block_on(check_download_version(
      mc, java, &data, &client, &handle, 3, None,
    ))
    .unwrap();
    assert_eq!(server.calls().len(), fetched + 1);
    assert_eq!(server.calls().last().unwrap().path, "/client.jar");
  }

  #[test]
  fn a_hash_mismatch_fails_and_writes_nothing() {
    let server = FakeServer::start();
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (handle, _rx) = DownloadsHandle::new();
    let fixture = rt::block_on(serve_version(&server));
    server.route("GET", "/client.jar", 200, "tampered");

    let mc = &fixture.manifest.versions[0];
    let result = rt::block_on(check_download_version(
      mc,
      &fixture.java.linux,
      &data,
      &default_client(),
      &handle,
      1,
      None,
    ));
    let err = result.unwrap_err();
    assert!(
      err
        .downcast_ref::<proton_utils::file::FileError>()
        .is_some(),
      "{err}"
    );
    assert!(!MCVersionPath::new(&data, "1.20.1").client_jar().exists());
  }

  #[test]
  fn http_errors_and_missing_runtimes_fail_the_download() {
    let server = FakeServer::start();
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (handle, _rx) = DownloadsHandle::new();
    let fixture = rt::block_on(serve_version(&server));
    let mc = &fixture.manifest.versions[0];
    let client = default_client();
    let run = |java: &PlatformVersion| {
      rt::block_on(check_download_version(
        mc, java, &data, &client, &handle, 1, None,
      ))
    };

    // no runtime for this component on this platform
    let empty = PlatformVersion::default();
    let err = run(&empty).unwrap_err();
    assert!(
      matches!(err.downcast_ref(), Some(DownloadError::NotSupported)),
      "{err}"
    );

    // a missing asset fails after the client is in place
    let version_path = MCVersionPath::new(&data, &mc.id);
    let version = rt::block_on(check_version_manifest(
      mc,
      &version_path,
      &client,
      &handle,
      0,
    ));
    let assets = rt::block_on(check_assets_manifest(
      &version.unwrap(),
      &MCPath::new(&data),
      &client,
      &handle,
      0,
    ));
    let hash = assets.unwrap().objects["a.txt"].hash.clone();
    let asset = format!("/resources.download.minecraft.net/{}/{hash}", &hash[..2]);
    server.route("GET", &asset, 500, "");
    assert!(run(&fixture.java.linux).is_err());
    assert!(version_path.client_jar().exists());
  }

  #[cfg(target_os = "linux")]
  #[test]
  fn a_loader_is_installed_after_the_game() {
    let server = FakeServer::start();
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (handle, rx) = DownloadsHandle::new();
    let fixture = rt::block_on(serve_version(&server));
    let meta = r#"{
      "loader": { "maven": "net.fabricmc:fabric-loader:0.15.0" },
      "launcherMeta": { "libraries": { "client": [], "common": [] }, "mainClass": { "client": "K" } }
    }"#;
    server
      .route(
        "GET",
        "/meta.fabricmc.net/v2/versions/loader/1.20.1/0.15.0",
        200,
        meta,
      )
      .route(
        "GET",
        "/maven.fabricmc.net/net/fabricmc/fabric-loader/0.15.0/fabric-loader-0.15.0.jar",
        200,
        "loader",
      );

    let loader = LoaderType::Fabric.loader_version("1.20.1".into(), "0.15.0".into());
    let mc = &fixture.manifest.versions[0];
    rt::block_on(check_download_version(
      mc,
      &fixture.java.linux,
      &data,
      &default_client(),
      &handle,
      1,
      loader,
    ))
    .unwrap();
    let jar = MCPath::new(&data)
      .library_path()
      .join("net/fabricmc/fabric-loader/0.15.0/fabric-loader-0.15.0.jar");
    assert_eq!(std::fs::read(jar).unwrap(), b"loader");

    assert_eq!(statuses(&rx).last(), Some(&DownloadCheckStatus::Done));
  }

  #[test]
  fn pools_propagate_the_first_error() {
    let (handle, rx) = DownloadsHandle::new();
    let checks = |fail: usize| -> Vec<_> {
      (0..3)
        .map(|i| async move {
          anyhow::ensure!(i != fail, "boom");
          anyhow::Ok((i != 1).then_some(i))
        })
        .collect()
    };
    rt::block_on(async {
      let status = DownloadCheckStatus::AssetsCheck;
      let mut found = check_pool(checks(9), &handle, 0, status).await.unwrap();
      found.sort();
      // checks that need nothing are dropped
      assert_eq!(found, [0, 2]);
      let err = check_pool(checks(2), &handle, 0, status).await.unwrap_err();
      assert_eq!(err.to_string(), "boom");

      let funcs = Vec::<(fn(_) -> std::future::Ready<Result<()>>, usize)>::new();
      download_pool(
        funcs,
        handle.clone(),
        5,
        DownloadCheckStatus::AssetsDownload,
      )
      .await
      .unwrap();
    });
    // an empty pool still reports 0 of 0 bytes
    let seen = statuses(&rx);
    assert!(
      seen.contains(&DownloadCheckStatus::AssetsDownload(0, 0)),
      "{seen:?}"
    );
  }
}
