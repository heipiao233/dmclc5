#![warn(missing_docs)]

//! A Minecraft launcher library.

use std::path::{Path, PathBuf};

use anyhow::{Ok, Result};
#[cfg(feature="msa_auth")]
use oauth2::{ClientId, DeviceAuthorizationUrl, EndpointNotSet, EndpointSet, TokenUrl, basic::BasicClient};
use reqwest::Client;

#[macro_use]
extern crate rust_i18n;

#[cfg(feature="content_services")]
pub mod content_services;
pub mod minecraft;
pub mod utils;
pub mod components;

i18n!("locales");

/// The core struct for DMCLC.
/// It contains everything we need.
pub struct LauncherConfig {
    name: String,
    assets_path: PathBuf,
    libraries_path: PathBuf,
    #[cfg(feature="msa_auth")]
    oauth2: BasicClient<EndpointNotSet, EndpointSet, EndpointNotSet, EndpointNotSet, EndpointSet>,
    http_client: Client,
    /// A HashMap of [AccountConstructor]s.
    // pub account_types: HashMap<String, Box<dyn AccountConstructor>>,
    /// Max download retry times.
    pub download_retries: usize,
    /// Max parallel downloading files.
    pub download_parallel_files: usize,
    /// BMCLAPI mirror.
    pub bmclapi_mirror: Option<String>
}

impl LauncherConfig {
    /// Creates a new [LauncherContext].
    ///
    /// # Arguments
    /// * `root_path` - The `.minecraft` directory.
    /// * `launcher_name` - Your launcher's name.
    #[cfg_attr(feature="msa_auth", doc=r" * `ms_client_id` - The client id for Microsoft auth. See [Microsoft's document](https://docs.microsoft.com/en-us/azure/active-directory/develop/quickstart-register-app).")]
    pub fn new(launcher_name: String, assets_path: PathBuf, libraries_path: PathBuf, #[cfg(feature="msa_auth")] ms_client_id: String) -> Result<Self> {
        #[allow(unused_mut)]
        let mut ctx = LauncherConfig {
            // root_path,
            assets_path: assets_path.canonicalize()?,
            libraries_path: libraries_path.canonicalize()?,
            http_client: Client::builder().user_agent(format!("{launcher_name}, based on heipiao233/dmclc5 (heipiao233@outlook.com)")).build()?,
            name: launcher_name,
            #[cfg(feature="msa_auth")]
            oauth2: oauth2::basic::BasicClient::new(ClientId::new(ms_client_id))
                .set_device_authorization_url(DeviceAuthorizationUrl::new("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode".to_string())?)
                .set_token_uri(TokenUrl::new("https://login.microsoftonline.com/consumers/oauth2/v2.0/token".to_string())?),
            download_retries: 5,
            download_parallel_files: 8,
            bmclapi_mirror: None
        };
        Ok(ctx)
    }

    /// Get a subpath in `libraries` dir
    pub fn get_libraries_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.libraries_path.join(path)
    }

    /// Get a subpath in `assets` dir
    pub fn get_assets_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.assets_path.join(path)
    }
}
