//! Things about authentication.

#[cfg(feature="msa_auth")]
pub mod microsoft;
pub mod yggdrasil;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::{Builder, Uuid};
use std::{collections::HashMap, ffi::OsString, fmt::Display};

use crate::{utils::BetterPath, LauncherContext};

use super::version::MinecraftInstallation;

/// An account.
#[async_trait]
pub trait Account: Display + Send + Sync {
    /// Check if it's initialized.
    fn is_initialized(&self) -> bool;

    /// Refresh access token and check if this account can be used now.
    /// If this account is not initialized it should return false;
    async fn check(&mut self, launcher: &LauncherContext) -> bool;

    /// Login. If [Account::check] returns `false` the client should call this.
    async fn login(&mut self, launcher: &LauncherContext) -> Result<()>;

    /// Get the account UUID.
    fn get_uuid(&self) -> Uuid;

    /// Prepare for launch.
    async fn prepare_launch(&self, version_launch_dir: &BetterPath, launcher: &LauncherContext) -> Result<()>;
    /// Get additional JVM arguments.
    async fn get_launch_jvmargs(&self, mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>>;
    /// Get additional game arguments.
    async fn get_launch_game_args(&mut self, launcher: &LauncherContext) -> HashMap<String, String>;
    /// Get log masks for security datas like access token, refresh token.
    /// If these strings appears in the log, the launcher should replace them with *** or other masks.
    fn get_log_masks(&self) -> Vec<String>;
}

/// A constructor for [Account].
pub trait AccountConstructor: Send + Sync {
    /// Create an empty [Account].
    fn new(&self) -> Box<dyn Account>;
    /// Create an [Account] from serialized data.
    fn deserialize(&self, de: &mut dyn erased_serde::Deserializer) -> Option<Box<dyn Account>>;
}


#[derive(Serialize, Deserialize)]
struct OfflineAccount(Option<String>);

pub(crate) struct OfflineAccountConstructor;

impl AccountConstructor for OfflineAccountConstructor {
    fn new(&self) -> Box<dyn Account> {
        Box::new(OfflineAccount(None))
    }

    fn deserialize(&self, de: &mut dyn erased_serde::Deserializer) -> Option<Box<dyn Account>> {
        Some(Box::new(erased_serde::deserialize::<OfflineAccount>(de).ok()?))
    }
}

impl Display for OfflineAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = &self.0 {
            write!(f, "{} ({})", v, t!("accounts.offline.name"))
        } else {
            write!(f, "{} {}", t!("accounts.uninitialized"), t!("accounts.offline.name"))
        }
    }
}

#[async_trait]
impl Account for OfflineAccount {
    fn is_initialized(&self) -> bool {
        self.0.is_some()
    }

    async fn check(&mut self, _: &LauncherContext) -> bool {
        self.0.is_some()
    }

    async fn login(&mut self, launcher: &LauncherContext) -> Result<()> {
        self.0 = Some(launcher.ui.ask_user_one(&t!("accounts.offline.username"), None).await.ok_or(anyhow!("User cancelled."))?);
        Ok(())
    }

    fn get_uuid(&self) -> Uuid {
        Builder::from_md5_bytes(md5::compute(self.0.as_ref().unwrap()).0).into_uuid()
    }

    async fn prepare_launch(&self, _: &BetterPath, _: &LauncherContext) -> Result<()> {
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _mc: &MinecraftInstallation, _: &LauncherContext) -> Result<Vec<OsString>> {
        Ok(vec![])
    }

    async fn get_launch_game_args(&mut self, _: &LauncherContext) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("${auth_access_token}".to_string(), "IT_WORKS".to_string());
        map.insert("${auth_session}".to_string(), "IT_WORKS".to_string());
        map.insert("${auth_player_name}".to_string(), self.0.clone().unwrap());
        map.insert("${user_type}".to_string(), "offline".to_string());
        map.insert("${user_properties}".to_string(), "{}".to_string());
        return map;
    }

    fn get_log_masks(&self) -> Vec<String> {
        vec![]
    }
}
