use std::path::PathBuf;

use anyhow::Result;
use url::Url;

use crate::paths::MCPath;
use proton_utils::path;

#[derive(Debug)]
pub struct MavenArtifact {
  pub group: String,
  pub artifact: String,
  pub version: String,
  pub version_ext: Option<String>,
  pub ext: String,
}

impl MavenArtifact {
  pub fn new(name: &str) -> Result<Self> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
      return Err(anyhow::anyhow!("Invalid Maven name format"));
    }

    let (version, version_ext, ext) = if parts.len() == 3 {
      let version_parts = parts[2].split('@').collect::<Vec<&str>>();

      let version = version_parts[0].to_string();
      let ext = if version_parts.len() > 1 {
        version_parts[1].to_string()
      } else {
        "jar".to_string()
      };

      (version, None, ext)
    } else {
      let version = parts[2].to_string();
      let version_ext_parts = parts[3].split('@').collect::<Vec<&str>>();

      let version_ext = version_ext_parts[0].to_string();
      let ext = if version_ext_parts.len() > 1 {
        version_ext_parts[1].to_string()
      } else {
        "jar".to_string()
      };

      (version, Some(version_ext), ext)
    };

    Ok(MavenArtifact {
      group: parts[0].to_string(),
      artifact: parts[1].to_string(),
      version,
      version_ext,
      ext,
    })
  }

  pub fn path(&self) -> PathBuf {
    let mut path = path!();
    let mut group: &str = &self.group;
    while let Some(segment) = group.find('.') {
      path = path!(path, &group[..segment]);
      group = &group[(segment + 1)..];
    }

    let version = if let Some(parts) = &self.version_ext {
      format!("{}-{}", self.version, parts)
    } else {
      self.version.clone()
    };

    path!(
      path,
      group,
      &self.artifact,
      &self.version,
      format!("{}-{}.{}", self.artifact, version, self.ext)
    )
  }

  pub fn full_path(&self, mc_path: &MCPath) -> PathBuf {
    path!(mc_path.library_path(), self.path())
  }

  pub fn url(&self, base_url: &str) -> Result<Url> {
    Ok(Url::parse(&format!(
      "{}/{}/{}/{}/{}-{}{}.{}",
      // `Url::as_str` keeps the trailing slash of a root url
      base_url.trim_end_matches('/'),
      self.group.replace('.', "/"),
      self.artifact,
      self.version,
      self.artifact,
      self.version,
      self
        .version_ext
        .as_deref()
        .map(|ext| format!("-{}", ext))
        .unwrap_or_default(),
      self.ext
    ))?)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn coordinates_parse_into_parts_paths_and_urls() {
    // (name, group, artifact, version, classifier, ext, path)
    let cases = [
      (
        "net.fabricmc:fabric-loader:0.15.0",
        "net.fabricmc",
        "fabric-loader",
        "0.15.0",
        None,
        "jar",
        "net/fabricmc/fabric-loader/0.15.0/fabric-loader-0.15.0.jar",
      ),
      (
        "org.lwjgl:lwjgl:3.3.1:natives-linux",
        "org.lwjgl",
        "lwjgl",
        "3.3.1",
        Some("natives-linux"),
        "jar",
        "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar",
      ),
      (
        "de.oceanlabs.mcp:mcp_config:1.20.1-20230612.114412@zip",
        "de.oceanlabs.mcp",
        "mcp_config",
        "1.20.1-20230612.114412",
        None,
        "zip",
        "de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412.zip",
      ),
      (
        "net.minecraft:client:1.20.1-20230612.114412:mappings@txt",
        "net.minecraft",
        "client",
        "1.20.1-20230612.114412",
        Some("mappings"),
        "txt",
        "net/minecraft/client/1.20.1-20230612.114412/client-1.20.1-20230612.114412-mappings.txt",
      ),
      // a group without dots is one dir
      (
        "solo:art:1",
        "solo",
        "art",
        "1",
        None,
        "jar",
        "solo/art/1/art-1.jar",
      ),
      // a fifth part is ignored
      (
        "a.b:c:1:x:y",
        "a.b",
        "c",
        "1",
        Some("x"),
        "jar",
        "a/b/c/1/c-1-x.jar",
      ),
      (
        "ünï.cödé:ärt:1.0",
        "ünï.cödé",
        "ärt",
        "1.0",
        None,
        "jar",
        "ünï/cödé/ärt/1.0/ärt-1.0.jar",
      ),
    ];
    for (name, group, artifact, version, classifier, ext, path) in cases {
      let maven = MavenArtifact::new(name).unwrap();
      assert_eq!(maven.group, group, "{name}");
      assert_eq!(maven.artifact, artifact, "{name}");
      assert_eq!(maven.version, version, "{name}");
      assert_eq!(maven.version_ext.as_deref(), classifier, "{name}");
      assert_eq!(maven.ext, ext, "{name}");
      assert_eq!(maven.path(), PathBuf::from(path), "{name}");
      let url = maven.url("https://repo.example/maven").unwrap();
      let expected = Url::parse(&format!("https://repo.example/maven/{path}")).unwrap();
      assert_eq!(url, expected, "{name}");
    }
  }

  #[test]
  fn malformed_coordinates_are_errors() {
    for name in ["", "a", "a:b", "nocolons.at.all"] {
      assert!(MavenArtifact::new(name).is_err(), "{name:?}");
    }
    let maven = MavenArtifact::new("a:b:1").unwrap();
    assert!(maven.url("not a url").is_err());
  }

  #[test]
  fn base_url_trailing_slash_does_not_double() {
    let maven = MavenArtifact::new("a.b:c:1").unwrap();
    // old forge profiles carry urls like `http://files.minecraftforge.net/maven/`
    let with = maven.url("https://repo.example/maven/").unwrap();
    let without = maven.url("https://repo.example/maven").unwrap();
    assert_eq!(with, without);
    assert_eq!(with.as_str(), "https://repo.example/maven/a/b/c/1/c-1.jar");
  }

  #[test]
  fn full_path_is_under_the_library_dir() {
    let data = PathBuf::from("/data dir/ü");
    let maven = MavenArtifact::new("a.b:c:1").unwrap();
    assert_eq!(
      maven.full_path(&MCPath::new(&data)),
      PathBuf::from("/data dir/ü/minecraft/lib/a/b/c/1/c-1.jar")
    );
  }
}
