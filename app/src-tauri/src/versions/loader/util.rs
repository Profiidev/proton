use std::path::Path;

use anyhow::Result;
use async_zip::base::read1::seek::ZipArchiveReader;
use reqwest::Client;
use tauri::Url;
use tokio::{
  fs,
  io::{AsyncReadExt, BufReader},
};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};

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
  let file = fs::File::open(zip_path).await?;
  let mut zip = ZipArchiveReader::open(BufReader::new(file).compat()).await?;

  let Some(index) = zip.find(file_name.as_bytes())?.next() else {
    return Err(anyhow::anyhow!("File '{}' not found in zip", file_name));
  };

  let mut bytes = Vec::new();
  zip
    .file(index)
    .await?
    .compat()
    .read_to_end(&mut bytes)
    .await?;
  Ok(bytes)
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

#[cfg(test)]
mod tests {
  use super::*;
  use async_zip::{Compression, ZipEntryBuilder, base::write::ZipFileWriter};
  use tokio_util::compat::TokioAsyncWriteCompatExt;

  #[tokio::test]
  async fn extract_file_from_zip_round_trips() {
    let dir = std::env::temp_dir().join(format!("proton-zip-test-{}", std::process::id()));
    fs::create_dir_all(&dir).await.unwrap();
    let zip_path = dir.join("t.zip");

    let out = fs::File::create(&zip_path).await.unwrap();
    let mut writer = ZipFileWriter::new(out.compat_write());
    for (name, body) in [("a.txt", b"hello".as_slice()), ("b/c.txt", b"world")] {
      let entry = ZipEntryBuilder::new(name.into(), Compression::Stored);
      writer.write_entry_whole(entry, body).await.unwrap();
    }
    writer.close().await.unwrap();

    assert_eq!(
      extract_file_from_zip(&zip_path, "b/c.txt").await.unwrap(),
      b"world"
    );
    assert!(extract_file_from_zip(&zip_path, "missing").await.is_err());

    fs::remove_dir_all(&dir).await.unwrap();
  }
}
