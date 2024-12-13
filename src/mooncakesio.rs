use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

const BASE_URL: &str = "https://moonbitlang-mooncakes.s3.us-west-2.amazonaws.com/user";

#[derive(Debug, thiserror::Error)]
pub enum MooncakesIOError {
    #[error("io error")]
    IOError(#[from] std::io::Error),
    #[error("return non zero")]
    ReturnNonZero(std::process::ExitStatus),
    #[error("from utf8")]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error("serde")]
    Serde(#[from] serde_json::Error),
    #[error("walkdir")]
    WalkDir(#[from] walkdir::Error),
}

pub fn download_to(name: &str, version: &str, dst: &Path) -> Result<(), MooncakesIOError> {
    let version_enc = form_urlencoded::Serializer::new(String::new())
        .append_key_only(version)
        .finish();
    let url = format!("{}/{}/{}.zip", BASE_URL, name, version_enc);
    let output_zip = format!("{}.zip", dst.join(version).display());

    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!("Invoke-WebRequest -Uri '{}' -OutFile '{}'", url, output_zip),
            ])
            .output()
            .map_err(|e| MooncakesIOError::IOError(e))?;
        if !output.status.success() {
            return Err(MooncakesIOError::ReturnNonZero(output.status));
        }

        let output = std::process::Command::new("powershell")
            .args([
                "-Command", 
                &format!("Expand-Archive -Path '{}' -DestinationPath '{}'", 
                    output_zip,
                    dst.join(version).display()
                ),
            ])
            .output()
            .map_err(|e| MooncakesIOError::IOError(e))?;
        if !output.status.success() {
            return Err(MooncakesIOError::ReturnNonZero(output.status));
        }
    }

    #[cfg(unix)]
    {
        let output = std::process::Command::new("curl")
            .arg("-o")
            .arg(&output_zip)
            .arg(&url)
            .output()
            .map_err(|e| MooncakesIOError::IOError(e))?;
        if !output.status.success() {
            return Err(MooncakesIOError::ReturnNonZero(output.status));
        }

        let output = std::process::Command::new("unzip")
            .arg(&output_zip)
            .arg("-d")
            .arg(dst.join(version))
            .output()
            .map_err(|e| MooncakesIOError::IOError(e))?;
        if !output.status.success() {
            return Err(MooncakesIOError::ReturnNonZero(output.status));
        }
    }

    Ok(())
}

pub fn home() -> PathBuf {
    if let Ok(moon_home) = std::env::var("MOON_HOME") {
        return PathBuf::from(moon_home);
    }

    let h = home::home_dir();
    if h.is_none() {
        eprintln!("Failed to get home directory");
        std::process::exit(1);
    }
    let hm = h.unwrap().join(".moon");
    if !hm.exists() {
        std::fs::create_dir_all(&hm).unwrap();
    }
    hm
}

pub fn index() -> PathBuf {
    home().join("registry").join("index")
}

pub fn index_of_pkg(base: &Path, user: &str, pkg: &str) -> PathBuf {
    base.join("user")
        .join(user)
        .join(pkg)
        .with_extension("index")
}

#[derive(Debug, Default)]
pub struct MooncakesDB {
    pub db: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
#[error("mooncakesdb error")]
pub struct MooncakesDBError {
    #[source]
    kind: MooncakesDBErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub enum MooncakesDBErrorKind {
    #[error("key not found: {key}")]
    NotFound { key: String },
}

impl MooncakesDB {
    pub fn get_latest_version(&self, name: &str) -> Result<String, MooncakesDBError> {
        self.db
            .get(name)
            .map(|versions| versions.last().unwrap().to_string())
            .ok_or(MooncakesDBError {
                kind: MooncakesDBErrorKind::NotFound {
                    key: name.to_string(),
                },
            })
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.db.contains_key(name)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct MooncakeInfo {
    version: String,
    keywords: Option<Vec<String>>,
}

#[test]
fn gen_latest_list() {
    let db = get_all_mooncakes().unwrap();
    for (name, versions) in db.db {
        let latest_version = versions.last().unwrap();
        println!("{} {}", name, latest_version);
    }
}

pub fn get_all_mooncakes() -> Result<MooncakesDB, MooncakesIOError> {
    let mut db: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let dir = index().join("user");
    let walker = walkdir::WalkDir::new(&dir).into_iter();
    for entry in walker.filter_map(|e| e.ok()).filter(|e| {
        e.path().is_file() && e.path().extension().and_then(|ext| ext.to_str()) == Some("index")
    }) {
        let p = entry.path();
        let name = p.strip_prefix(&dir).unwrap().to_str().unwrap();
        let dot_index = name.rfind(".index").unwrap_or(name.len());
        let name = &name[0..dot_index];

        let index_file_content =
            std::fs::read_to_string(entry.path()).map_err(|e| MooncakesIOError::IOError(e))?;
        let mut is_mooncakes_test = false;
        let mut indexes = vec![];
        for line in index_file_content.lines() {
            let index: MooncakeInfo =
                serde_json::from_str(line).map_err(|e| MooncakesIOError::Serde(e))?;
            indexes.push(index.version);
            if let Some(keywords) = &index.keywords {
                if keywords.contains(&"mooncakes-test".to_string()) {
                    is_mooncakes_test = true;
                }
            }
        }
        if !is_mooncakes_test {
            db.insert(name.to_string(), indexes);
        }
    }
    Ok(MooncakesDB { db })
}
