//! Manage game dirs

use std::{fs, path::{Path, PathBuf}};



use crate::{LauncherConfig, errors::Result, minecraft::{schemas::VersionJSON, version::MinecraftInstallation}, utils::merge_version_json};

/// A game prefix, typically .minecraft
pub struct MinecraftPrefix {
    /// Prefix path
    path: PathBuf,
    /// Prefix path / versions
    versions_path: PathBuf,
}

impl MinecraftPrefix {
    /// Create a new prefix
    pub fn new(path: PathBuf) -> Result<Self> {
        let path = fs::canonicalize(path)?;
        Ok(Self {
            versions_path: path.join("versions"),
            path
        })
    }
    /// List minecraft installations in this prefix.
    pub fn list_installations<'c>(&self, config: &'c LauncherConfig) -> Result<Vec<MinecraftInstallation<'c>>> {
        fs::create_dir_all(self.versions_path())?;
        Ok(fs::read_dir(self.versions_path())?
            .filter_map(|dir| dir.ok())
            .map(|dir| dir.file_name().to_string_lossy().to_string())
            .filter_map(|dir| self.get_installation(&dir, config))
            .collect()
        )
    }
    
    /// Get one [MinecraftInstallation] by name in this prefix.
    pub fn get_installation<'c>(&self, name: &str, config: &'c LauncherConfig) -> Option<MinecraftInstallation<'c>> {
        let version_dir = self.versions_path().join(name);
        let meta = fs::metadata(&version_dir);
        if meta.is_err() || !meta.unwrap().is_dir() {
            return None;
        }
        let json = fs::File::open(version_dir.join(name.to_string() + ".json")).ok()?;
        let json = serde_json::from_reader(&json).ok()?;
        let json: VersionJSON = self.resolve_inherits_from(json);
        Some(MinecraftInstallation::new(self, config, json, name, None))
    }
    
    fn resolve_inherits_from(&self, base: VersionJSON) -> VersionJSON {
        let mut current = base;
        while let Some(father) = &current.get_base().inherits_from {
            let version_dir = self.versions_path().join(father);
            let father = fs::read(version_dir.join(father.to_string() + ".json")).ok();
            if father.is_none() {
                return current;
            }
            let father = serde_json::from_slice(&father.unwrap());
            if father.is_err() {
                return current;
            }
            current = if let Ok(current) = merge_version_json(&father.unwrap(), &current) {
                current
            } else {
                return current;
            };
        }
        current
    }

    /// Returns the path of `versions`
    pub fn versions_path(&self) -> &Path {
        &self.versions_path
    }

    /// Returns the path of this prefix
    pub fn path(&self) -> &Path {
        &self.path
    }
}
