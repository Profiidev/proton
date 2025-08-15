use std::path::Path;

use anyhow::Result;
use async_zip::tokio::read::fs::ZipFileReader;
use reqwest::Client;

use crate::{
  utils::file::{download_file_no_hash_force, file_hash},
  versions::{
    loader::{CheckFuture, DownloadFuture},
    maven::{full_path_from_maven, parse_maven_name, url_from_maven},
    paths::MCPath,
  },
};

pub fn download_maven_future(
  mc_path: MCPath,
  name: String,
  client: Client,
  base_url: String,
  sha1: Option<String>,
) -> CheckFuture {
  Box::pin(async move {
    let local_name = name.clone();
    let mc = mc_path.clone();
    let download = Box::pin(async move {
      download_maven(&base_url, &mc, &local_name, &client).await?;
      anyhow::Ok(())
    }) as DownloadFuture;

    let maven = parse_maven_name(&name)?;
    let path = full_path_from_maven(&mc_path, &maven);
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

pub async fn download_maven(
  base_url: &str,
  mc_path: &MCPath,
  name: &str,
  client: &Client,
) -> Result<()> {
  let maven = parse_maven_name(name)?;
  let loader_path = full_path_from_maven(mc_path, &maven);
  let loader_url = url_from_maven(base_url, &maven)?;
  download_file_no_hash_force(client, &loader_path, loader_url).await?;
  Ok(())
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
