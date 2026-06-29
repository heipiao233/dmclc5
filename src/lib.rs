#![warn(missing_docs)]

//! A Minecraft launcher library.

use std::{io::Write, path::Path, sync::Arc};

use anyhow::{Ok, Result};
use minecraft::{schemas::VersionJSON, version::MinecraftInstallation};
#[cfg(feature="msa_auth")]
use oauth2::{ClientId, DeviceAuthorizationUrl, EndpointNotSet, EndpointSet, TokenUrl, basic::BasicClient};
use reqwest::Client;
use tokio::{fs::{self, create_dir_all}, io::AsyncWriteExt};
use utils::{osstr_concat, BetterPathBuf};

use crate::utils::merge_version_json;
#[macro_use]
extern crate rust_i18n;

#[cfg(feature="content_services")]
pub mod content_services;
pub mod minecraft;
pub mod utils;
pub mod components;

i18n!("locales");

const CACHEDIR_TAG: &str = r"Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by a Minecraft launcher.
# For information about cache directory tags, see:
#	http://www.brynosaurus.com/cachedir/";
/// The core struct for DMCLC.
/// It contains everything we need.
pub struct LauncherContext {
    root_path: BetterPathBuf,
    name: String,
    #[cfg(feature="msa_auth")]
    oauth2: BasicClient<EndpointNotSet, EndpointSet, EndpointNotSet, EndpointNotSet, EndpointSet>,
    http_client: Client,
    /// A HashMap of [AccountConstructor]s.
    // pub account_types: HashMap<String, Box<dyn AccountConstructor>>,
    /// Max download retry times.
    pub download_retries: usize,
    /// Max download threads per file.
    pub download_threads_per_file: u16,
    /// Max parallel downloading files.
    pub download_parallel_files: usize,
    /// BMCLAPI mirror.
    pub bmclapi_mirror: Option<String>
}

impl LauncherContext {

    /// Creates a new [LauncherContext].
    ///
    /// # Arguments
    /// * `root_path` - The `.minecraft` directory.
    /// * `launcher_name` - Your launcher's name.
    #[cfg_attr(feature="msa_auth", doc=r" * `ms_client_id` - The client id for Microsoft auth. See [Microsoft's document](https://docs.microsoft.com/en-us/azure/active-directory/develop/quickstart-register-app).")]
    pub async fn new(mc_path: &Path, launcher_name: String, #[cfg(feature="msa_auth")] ms_client_id: String) -> Result<Self> {
        let root_path: BetterPathBuf;
        if let Err(_) | Result::Ok(false) = tokio::fs::try_exists(&mc_path).await {
            tokio::fs::create_dir_all(&mc_path).await?;
            root_path = BetterPathBuf(mc_path.to_path_buf().canonicalize()?);
            tokio::fs::create_dir(root_path.clone() / "libraries").await?;
            tokio::fs::create_dir(root_path.clone() / "assets").await?;
        } else {
            root_path = BetterPathBuf(mc_path.to_path_buf().canonicalize()?);
            if let Err(_) | Result::Ok(false) = tokio::fs::try_exists(root_path.clone() / "libraries").await {
                tokio::fs::create_dir_all(root_path.clone() / "libraries").await?;
            }
            if let Err(_) | Result::Ok(false) = tokio::fs::try_exists(root_path.clone() / "assets").await {
                tokio::fs::create_dir(root_path.clone() / "assets").await?;
            }
        }
        tokio::fs::File::create(root_path.clone() / "libraries" / "CACHEDIR.TAG").await?.write_all(CACHEDIR_TAG.as_bytes()).await?;
        tokio::fs::File::create(root_path.clone() / "assets" / "CACHEDIR.TAG").await?.write_all(CACHEDIR_TAG.as_bytes()).await?;
        #[allow(unused_mut)]
        let mut ctx = LauncherContext {
            root_path,
            http_client: Client::builder().user_agent(format!("{launcher_name}, based on heipiao233/dmclc5 (heipiao233@outlook.com)")).build()?,
            name: launcher_name,
            #[cfg(feature="msa_auth")]
            oauth2: oauth2::basic::BasicClient::new(ClientId::new(ms_client_id))
                .set_device_authorization_url(DeviceAuthorizationUrl::new("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode".to_string())?)
                .set_token_uri(TokenUrl::new("https://login.microsoftonline.com/consumers/oauth2/v2.0/token".to_string())?),
            download_retries: 5,
            download_threads_per_file: 8,
            download_parallel_files: 8,
            bmclapi_mirror: None
        };
        Ok(ctx)
    }

    /// List the names of minecraft installations in the `root_path`.
    pub async fn list_installations(&self) -> Result<Vec<String>> {
        let version_dir = self.root_path.clone() / "versions";
        let mut ret = Vec::new();
        create_dir_all(&version_dir).await?;
        for i in std::fs::read_dir(self.root_path.clone() / "versions")? {
            let dir = i?;
            if !dir.file_type()?.is_dir() {
                continue;
            }
            let json_path = version_dir.clone() / dir.file_name() / osstr_concat(&dir.file_name(), &".json".to_string());
            if let Result::Ok(meta) = std::fs::metadata(&json_path) && meta.is_file() {
                ret.push(dir.file_name().to_string_lossy().to_string());
            }
        }
        Ok(ret)
    }

    /// Get one [MinecraftInstallation] by name in the `root_path`.
    pub async fn get_installation(self: Arc<Self>, name: &str) -> Option<MinecraftInstallation> {
        let version_dir = self.root_path.clone() / "versions" / name;
        let meta = fs::metadata(&version_dir).await;
        if meta.is_err() || !meta.unwrap().is_dir() {
            return None;
        }
        let json = fs::read(version_dir / (name.to_string() + ".json")).await.ok()?;
        let json = serde_json::from_slice(&json).ok()?;
        let json: VersionJSON = self.resolve_inherits_from(json).await;
        Some(MinecraftInstallation::new(self, json, name, None))
    }

    async fn resolve_inherits_from(&self, base: VersionJSON) -> VersionJSON {
        let mut current = base;
        while let Some(father) = &current.get_base().inherits_from {
            let version_dir = self.root_path.clone() / "versions" / father;
            let father = fs::read(version_dir / (father.to_string() + ".json")).await.ok();
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

    /// Set a new `root_path`.
    pub fn set_root_path(&mut self, root_path: &Path) -> Result<()> {
        self.root_path = BetterPathBuf(root_path.to_path_buf().canonicalize()?);
        std::fs::File::create(self.root_path.clone() / "libraries" / "CACHEDIR.TAG")?.write_all(CACHEDIR_TAG.as_bytes())?;
        std::fs::File::create(self.root_path.clone() / "assets" / "CACHEDIR.TAG")?.write_all(CACHEDIR_TAG.as_bytes())?;
        Ok(())
    }
}
