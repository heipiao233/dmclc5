//! Manage game dirs

use std::{fs, path::{Path, PathBuf}};

use anyhow::Result;

use crate::{LauncherConfig, minecraft::{schemas::VersionJSON, version::MinecraftInstallation}, utils::{merge_version_json, osstr_concat}};

/// A game prefix, typically .minecraft
pub struct MinecraftPrefix<'c> {
    /// Prefix path
    path: PathBuf,
    /// Prefix path / versions
    versions_path: PathBuf,
    /// Launcher config of this prefix
    pub config: &'c LauncherConfig
}

impl <'c> MinecraftPrefix<'c> {
    /// Create a new prefix
    pub fn new(path: PathBuf, config: &'c LauncherConfig) -> Result<Self> {
        let path = fs::canonicalize(path)?;
        Ok(Self {
            versions_path: path.join("versions"),
            path,
            config
        })
    }
    /// List the names of minecraft installations in the `root_path`.
    pub fn list_installations(&self) -> Result<Vec<String>> {
        let mut ret = Vec::new();
        fs::create_dir_all(self.versions_path())?;
        for i in fs::read_dir(self.versions_path())? {
            let dir = i?;
            if !dir.file_type()?.is_dir() {
                continue;
            }
            let json_path = self.versions_path().join(dir.file_name()).join(osstr_concat(&dir.file_name(), &".json".to_string()));
            if let Result::Ok(meta) = fs::metadata(&json_path) && meta.is_file() {
                ret.push(dir.file_name().to_string_lossy().to_string());
            }
        }
        Ok(ret)
    }
    
    /// Get one [MinecraftInstallation] by name in the `root_path`.
    pub fn get_installation<'p>(&'p self, name: &str) -> Option<MinecraftInstallation<'c, 'p>> {
        let version_dir = self.versions_path().join(name);
        let meta = fs::metadata(&version_dir);
        if meta.is_err() || !meta.unwrap().is_dir() {
            return None;
        }
        let json = fs::read(version_dir.join(name.to_string() + ".json")).ok()?;
        let json = serde_json::from_slice(&json).ok()?;
        let json: VersionJSON = self.resolve_inherits_from(json);
        Some(MinecraftInstallation::new(self, json, name, None))
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
            current = if let Result::Ok(current) = merge_version_json(&father.unwrap(), &current) {
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
