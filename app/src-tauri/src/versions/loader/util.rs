use std::path::{Path, PathBuf};

use anyhow::Result;
use reqwest::Client;

use crate::{
  utils::file::{download_file_no_hash_force, file_hash},
  versions::{
    loader::{CheckFuture, DownloadFuture},
    maven::{full_path_from_maven, parse_maven_name, url_from_maven},
  },
};

pub fn download_maven_future(
  data_dir: PathBuf,
  name: String,
  client: Client,
  base_url: String,
  sha1: Option<String>,
) -> CheckFuture {
  Box::pin(async move {
    let local_name = name.clone();
    let data = data_dir.clone();
    let download = Box::pin(async move {
      download_maven(&base_url, &data_dir, &local_name, &client).await?;
      anyhow::Ok(())
    }) as DownloadFuture;

    let maven = parse_maven_name(&name);
    let path = full_path_from_maven(&data, &maven);
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
  data_dir: &Path,
  name: &str,
  client: &Client,
) -> Result<()> {
  let maven = parse_maven_name(name);
  let loader_path = full_path_from_maven(data_dir, &maven);
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
