use std::{future::Future, path::PathBuf};

use anyhow::Result;
use log::debug;
use reqwest::Client;

use crate::{
  path,
  utils::{
    file::{download_and_parse_file, download_file, file_hash, read_parse_file},
    future::DataOrFuture,
  },
  versions::{
    download::DownloadError,
    meta::{
      java::{Component, Files, PlatformVersion},
      minecraft::{Assets, ManifestVersion, Version},
    },
    ASSETS_DIR, ASSETS_INDEX_DIR, JAVA_DIR, MC_DIR, VERSION_DIR,
  },
};

pub async fn check_version_manifest(
  info: &ManifestVersion,
  data_dir: &PathBuf,
  client: &Client,
) -> Result<DataOrFuture<Version>> {
  let path = path!(
    data_dir,
    MC_DIR,
    VERSION_DIR,
    &info.id,
    format!("{}.json", info.id)
  );

  debug!("Checking minecraft manifest for version {}", info.id);
  if !file_hash(&info.sha1, &path).await? {
    let client = client.clone();
    let info = info.clone();
    return Ok(DataOrFuture::future(async move {
      debug!("Downloading minecraft manifest for version {}", info.id);
      download_and_parse_file(&client, &path, info.url.clone(), &info.sha1).await
    }));
  }

  Ok(DataOrFuture::data(read_parse_file(&path).await?))
}

pub async fn check_assets_manifest(
  info: &Version,
  data_dir: &PathBuf,
  client: &Client,
) -> Result<DataOrFuture<Assets>> {
  let assets_index = &info.asset_index;
  let path = path!(
    data_dir,
    MC_DIR,
    ASSETS_DIR,
    ASSETS_INDEX_DIR,
    format!("{}.json", assets_index.id)
  );

  debug!("Checking assets manifest {}", assets_index.id);
  if !file_hash(&assets_index.sha1, &path).await? {
    let client = client.clone();
    let assets_index = assets_index.clone();
    return Ok(DataOrFuture::future(async move {
      debug!("Downloading assets manifest {}", assets_index.id);
      download_and_parse_file(&client, &path, assets_index.url.clone(), &assets_index.sha1).await
    }));
  }

  Ok(DataOrFuture::data(read_parse_file(&path).await?))
}

pub async fn check_java_manifest(
  info: &Version,
  version: &PlatformVersion,
  data_dir: &PathBuf,
  client: &Client,
) -> Result<DataOrFuture<(Files, Component)>> {
  let java_version = &info.java_version;
  let java_component = &java_version.component;

  let list = match java_component {
    Component::JavaRuntimeAlpha => &version.java_runtime_alpha,
    Component::JavaRuntimeBeta => &version.java_runtime_beta,
    Component::JavaRuntimeDelta => &version.java_runtime_delta,
    Component::JavaRuntimeGamma => &version.java_runtime_gamma,
    Component::JavaRuntimeGammaSnapshot => &version.java_runtime_gamma_snapshot,
    Component::JreLegacy => &version.jre_legacy,
    _ => return Err(DownloadError::NotSupported.into()),
  };
  let Some(version) = list.first() else {
    return Err(DownloadError::NotSupported.into());
  };

  let id = java_component.to_string();
  let path = path!(data_dir, JAVA_DIR, &id, format!("{}.json", &id));

  let download = &version.manifest;
  debug!("Checking java manifest for {id}");
  if !file_hash(&download.sha1, &path).await? {
    let client = client.clone();
    let download = download.clone();
    let java_component = *java_component;
    return Ok(DataOrFuture::future(async move {
      debug!("Downloading java manifest for {id}");
      Ok((
        download_and_parse_file(&client, &path, download.url.clone(), &download.sha1).await?,
        java_component,
      ))
    }));
  }

  let files = read_parse_file(&path).await?;
  Ok(DataOrFuture::data((files, *java_component)))
}

pub async fn check_client(
  version: &Version,
  data_dir: &PathBuf,
  client: &Client,
) -> Result<Option<impl Future<Output = Result<()>> + Send>> {
  let download = &version.downloads.client;
  let path = path!(
    data_dir,
    MC_DIR,
    VERSION_DIR,
    &version.id,
    format!("{}.jar", version.id)
  );

  debug!("Checking client jar for version {}", version.id);
  if !file_hash(&download.sha1, &path).await? {
    let client = client.clone();
    let download = download.clone();
    let id = version.id.clone();
    return Ok(Some(async move {
      debug!("Downloading client jar for version {id}");
      download_file(&client, &path, download.url.clone(), &download.sha1).await?;
      Ok(())
    }));
  }

  Ok(None)
}
