//! Things about authentication.

#[cfg(feature="msa_auth")]
pub mod microsoft;
pub mod yggdrasil;
pub mod offline;

use std::{collections::HashMap, ffi::OsString, fmt::Display};

use anyhow::Result;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{LauncherContext, minecraft::login::{offline::OfflineAccount, yggdrasil::YggdrasilAccount}, utils::BetterPathBuf};
#[cfg(feature = "msa_auth")]
use crate::minecraft::login::microsoft::MicrosoftAccount;

use super::version::MinecraftInstallation;

/// An account.
#[enum_dispatch]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
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
            Self::YggdrasilAccount(account) => account.fmt(f),
            #[cfg(feature = "msa_auth")]
            Self::MicrosoftAccount(account) => account.fmt(f)
        }
    }
}

/// The interface of [Account]
#[enum_dispatch(Account)]
#[allow(async_fn_in_trait)]
pub trait AccountTrait: Display + Sized {

    /// Refresh access token and check if this account can be used now.
    /// If this account is not initialized it should return false;
    async fn check(self, launcher: &LauncherContext) -> Result<Account>;

    /// Get the account UUID.
    fn get_uuid(&self) -> Uuid;

    /// Prepare for launch.
    async fn prepare_launch(&self, version_launch_dir: &BetterPathBuf, launcher: &LauncherContext) -> Result<()>;
    /// Get additional JVM arguments.
    async fn get_launch_jvmargs(&self, mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>>;
    /// Get additional game arguments.
    fn replace_launch_game_arg(&self, arg: &String) -> String;
    /// Get log masks for security datas like access token, refresh token.
    /// If these strings appears in the log, the launcher should replace them with *** or other masks.
    fn get_log_masks(&self) -> Vec<String>;
}
