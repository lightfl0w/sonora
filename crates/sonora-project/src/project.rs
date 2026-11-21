use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sonora_core::{Scale, Song};
use sonora_engine::StoredVoice;

use crate::error::ProjectError;

pub const FORMAT: &str = "sonora-project";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "sonora";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub title: String,
    pub song: Song,
    pub scale: Scale,
    #[serde(default)]
    pub voices: Vec<StoredVoice>,
}

#[derive(Deserialize)]
struct Probe {
    format: Option<String>,
    version: Option<u32>,
}

impl Default for Project {
    fn default() -> Self {
        Self::empty()
    }
}

impl Project {
    pub fn empty() -> Self {
        Self {
            format: FORMAT.to_string(),
            version: VERSION,
            title: "未命名工程".to_string(),
            song: Song::empty(),
            scale: Scale::default(),
            voices: Vec::new(),
        }
    }

    pub fn with_song(song: Song, scale: Scale) -> Self {
        Self {
            song,
            scale,
            ..Self::empty()
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, ProjectError> {
        let probe: Probe = serde_json::from_slice(data)?;

        match probe.format.as_deref() {
            Some(FORMAT) => {}
            Some(_) => {
                return Err(ProjectError::WrongFormat {
                    expected: FORMAT.to_string(),
                    found: probe.format.unwrap_or_default(),
                });
            }
            None => return Err(ProjectError::NotProject),
        }

        let found = probe.version.unwrap_or(0);
        if found > VERSION {
            return Err(ProjectError::NewerVersion {
                supported: VERSION,
                found,
            });
        }

        let mut project: Project = serde_json::from_slice(data)?;
        project.format = FORMAT.to_string();
        project.version = VERSION;
        project.song.sanitize();
        if project.title.trim().is_empty() {
            project.title = "未命名工程".to_string();
        }
        Ok(project)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, ProjectError> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let data = std::fs::read(path).map_err(|source| ProjectError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_bytes(&data)
    }

    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        let write_err = |source: std::io::Error| ProjectError::Write {
            path: path.to_path_buf(),
            source,
        };

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(write_err)?;
            }
        }

        let data = self.to_bytes()?;
        let tmp = temp_path(path);
        std::fs::write(&tmp, &data).map_err(write_err)?;
        std::fs::rename(&tmp, path).map_err(|source| {
            let _ = std::fs::remove_file(&tmp);
            ProjectError::Write {
                path: path.to_path_buf(),
                source,
            }
        })
    }

    pub fn display_name(&self) -> String {
        if self.title.trim().is_empty() {
            "未命名工程".to_string()
        } else {
            self.title.clone()
        }
    }
}

pub fn with_extension(path: &Path) -> PathBuf {
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension(EXTENSION)
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}
