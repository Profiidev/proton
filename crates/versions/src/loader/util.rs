use std::path::Path;

use anyhow::Result;
use async_zip::base::read1::seek::ZipArchiveReader;
use reqwest::Client;
use tokio::{
  fs,
  io::{AsyncReadExt, BufReader},
};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use url::Url;

use crate::{
  loader::{CheckFuture, DownloadFuture},
  maven::MavenArtifact,
  paths::MCPath,
};
use proton_utils::{
  download::{DownloadFileSizeFuture, download_file_size},
  file::file_hash,
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

  #[test]
  fn mc_versions_compare_numerically() {
    use std::cmp::Ordering::*;
    let cases = [
      ("1.5.1", "1.5.2", Less),
      ("1.10", "1.9", Greater),
      ("1.20.1", "1.20.1", Equal),
      // more parts is newer
      ("1.20", "1.20.1", Less),
      ("1.20.1", "1.20", Greater),
      // a non numeric part sorts below a number, whichever side it is on
      ("1.7.10_pre4", "1.7.10", Less),
      ("1.7.10", "1.7.10_pre4", Greater),
      ("1.5.1", "1.7.10_pre4", Less),
      ("", "1", Less),
    ];
    for (a, b, expected) in cases {
      assert_eq!(
        compare_mc_versions(&a.into(), &b.into()),
        expected,
        "{a} vs {b}"
      );
    }
  }

  #[test]
  fn main_class_comes_from_the_jar_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let jar = dir.path().join("p.jar");
    let manifest = b"Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.Installer\r\n";
    proton_utils::rt::block_on(async {
      crate::testing::write_zip(&jar, &[("META-INF/MANIFEST.MF", manifest)]).await;
      // lines() drops the \r too
      assert_eq!(
        main_class_from_jar(&jar).await.unwrap(),
        "net.minecraftforge.Installer"
      );

      let no_main = dir.path().join("n.jar");
      crate::testing::write_zip(
        &no_main,
        &[("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\n")],
      )
      .await;
      let err = main_class_from_jar(&no_main).await.unwrap_err();
      assert_eq!(err.to_string(), "Main-Class not found");

      let not_utf8 = dir.path().join("u.jar");
      crate::testing::write_zip(&not_utf8, &[("META-INF/MANIFEST.MF", &[0xff, 0xfe])]).await;
      assert!(main_class_from_jar(&not_utf8).await.is_err());
      assert!(
        main_class_from_jar(&dir.path().join("missing.jar"))
          .await
          .is_err()
      );

      fs::write(dir.path().join("bad.jar"), b"not a zip")
        .await
        .unwrap();
      assert!(
        main_class_from_jar(&dir.path().join("bad.jar"))
          .await
          .is_err()
      );
    });
  }

  #[test]
  fn extract_and_save_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("z.zip");
    let target = dir.path().join("a/b/ü c.txt");
    proton_utils::rt::block_on(async {
      crate::testing::write_zip(&zip, &[("x.txt", b"x")]).await;
      extract_and_save_file_from_zip(&zip, "x.txt", &target)
        .await
        .unwrap();
      assert_eq!(fs::read(&target).await.unwrap(), b"x");
      // a missing entry writes nothing
      let other = dir.path().join("o/o.txt");
      assert!(
        extract_and_save_file_from_zip(&zip, "y.txt", &other)
          .await
          .is_err()
      );
      assert!(!other.exists());
    });
  }
}
