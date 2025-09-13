use std::path::Path;

use anyhow::Result;
use async_zip::tokio::read::fs::ZipFileReader;
use reqwest::Client;
use tauri::Url;
use tokio::fs;

use crate::{
  utils::{
    download::{DownloadFileSizeFuture, download_file_size},
    file::file_hash,
  },
  versions::{
    loader::{CheckFuture, DownloadFuture},
    maven::MavenArtifact,
    paths::MCPath,
  },
};

pub fn download_maven_future(
  mc_path: MCPath,
  name: String,
  client: Client,
  base_url: String,
  sha1: Option<String>,
  url: Option<Url>,
) -> CheckFuture {
  Box::pin(async move {
    let local_name = name.clone();
    let mc = mc_path.clone();
    let download =
      Box::pin(async move { download_maven(&base_url, &mc, &local_name, &client, url).await })
        as DownloadFuture;

    let maven = MavenArtifact::new(&name)?;
    let path = maven.full_path(&mc_path);
    if let Some(sha1) = sha1 {
      if !file_hash(&sha1, &path).await? {
        Ok(Some(download))
      } else {
        Ok(None)
      }
    } else {
      Ok(Some(download))
    }
  }) as CheckFuture
}

async fn download_maven(
  base_url: &str,
  mc_path: &MCPath,
  name: &str,
  client: &Client,
  url: Option<Url>,
) -> Result<(DownloadFileSizeFuture, usize)> {
  let maven = MavenArtifact::new(name)?;
  let loader_path = maven.full_path(mc_path);
  let loader_url = url.unwrap_or_else(|| maven.url(base_url).unwrap());
  download_file_size(client, loader_path, loader_url).await
}

#[allow(clippy::ptr_arg)]
pub fn compare_mc_versions(a: &String, b: &String) -> std::cmp::Ordering {
  let a_parts: Vec<&str> = a.split('.').collect();
  let b_parts: Vec<&str> = b.split('.').collect();

  for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
    match a_part.parse::<u32>() {
      Ok(a_num) => match b_part.parse::<u32>() {
        Ok(b_num) => {
          if a_num != b_num {
            return a_num.cmp(&b_num);
          }
        }
        Err(_) => return std::cmp::Ordering::Greater,
      },
      Err(_) => return std::cmp::Ordering::Less,
    }
  }

  a_parts.len().cmp(&b_parts.len())
}

pub async fn extract_file_from_zip(zip_path: &Path, file_name: &str) -> Result<Vec<u8>> {
  let zip = ZipFileReader::new(zip_path).await?;
  let mut data = None;

  for i in 0..zip.file().entries().len() {
    let mut reader = zip.reader_with_entry(i).await?;
    let entry = reader.entry();

    if entry.filename().as_str().unwrap_or_default() == file_name {
      let mut bytes = Vec::new();
      reader.read_to_end_checked(&mut bytes).await?;
      data = Some(bytes);
      break;
    }
  }

  if let Some(bytes) = data {
    Ok(bytes)
  } else {
    Err(anyhow::anyhow!("File '{}' not found in zip", file_name))
  }
}

pub async fn extract_and_save_file_from_zip(
  zip_path: &Path,
  file_name: &str,
  save_path: &Path,
) -> Result<()> {
  let data = extract_file_from_zip(zip_path, file_name).await?;
  let parent = save_path
    .parent()
    .ok_or_else(|| anyhow::anyhow!("Invalid save path"))?;
  fs::create_dir_all(parent).await?;
  fs::write(save_path, data).await?;
  Ok(())
}

pub async fn main_class_from_jar(jar_path: &Path) -> Result<String> {
  //find Main-Class in the jar
  let manifest_data = extract_file_from_zip(jar_path, "META-INF/MANIFEST.MF").await?;
  let manifest = String::from_utf8(manifest_data)?;
  let mut main_class = None;
  for line in manifest.lines() {
    if line.starts_with("Main-Class: ") {
      main_class = Some(line.strip_prefix("Main-Class: ").unwrap().to_string());
      break;
    }
  }
  main_class.ok_or_else(|| anyhow::anyhow!("Main-Class not found"))
}
