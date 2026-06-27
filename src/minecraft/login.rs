//! Things about authentication.

#[cfg(feature="msa_auth")]
pub mod microsoft;
pub mod yggdrasil;
pub mod offline;

use std::{collections::HashMap, ffi::OsString, fmt::Display};

use anyhow::Result;
use enum_dispatch::enum_dispatch;
use uuid::Uuid;

use crate::{LauncherContext, minecraft::login::{offline::OfflineAccount, yggdrasil::YggdrasilAccount}, utils::BetterPath};
#[cfg(feature = "msa_auth")]
use crate::minecraft::launch::msa::MicrosoftAccount;

use super::version::MinecraftInstallation;

/// An account.
#[enum_dispatch]
pub enum Account {
    /// Offline account. Please take care of anti priate.
    OfflineAccount,
    /// Yggdrasil account.
    YggdrasilAccount,
    #[cfg(feature = "msa_auth")]
    /// Microsoft account. This is the only one that proves the player has bought Minecraft.
    MicrosoftAccount
}

impl Display for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OfflineAccount(account) => account.fmt(f),
            Self::YggdrasilAccount(account) => account.fmt(f)
        }
    }
}

/// The interface of [Account]
#[enum_dispatch(Account)]
#[allow(async_fn_in_trait)]
pub trait AccountTrait: Display {

    /// Refresh access token and check if this account can be used now.
    /// If this account is not initialized it should return false;
    async fn check(&mut self, launcher: &LauncherContext) -> bool;

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
