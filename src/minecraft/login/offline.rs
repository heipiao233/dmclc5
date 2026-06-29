//! The authentication method that allows offline launch.
//! Please take care of anti priate.

use std::{ffi::OsString, fmt::Display};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::{Builder, Uuid};

use crate::{LauncherContext, minecraft::{login::{Account, AccountTrait}, version::MinecraftInstallation}, utils::BetterPathBuf};


/// An offline account, useful if no Internet.
#[derive(Serialize, Deserialize)]
pub struct OfflineAccount(pub String, Uuid);

impl Display for OfflineAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.0, t!("accounts.offline.name"))
    }
}

impl OfflineAccount {
    pub fn new(name: String) -> Self {
        let uuid = Builder::from_md5_bytes(md5::compute(&name).0).into_uuid();
        Self(name, uuid)
    }
}

impl AccountTrait for OfflineAccount {
    async fn check(self, _: &LauncherContext) -> Result<Account> {
        Ok(self.into())
    }

    fn get_uuid(&self) -> Uuid {
        self.1
    }

    async fn prepare_launch(&self, _: &BetterPathBuf, _: &LauncherContext) -> Result<()> {
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _mc: &MinecraftInstallation, _: &LauncherContext) -> Result<Vec<OsString>> {
        Ok(vec![])
    }

    fn replace_launch_game_arg(&self, arg: &String) -> String {
        arg.replace("${auth_player_name}", &self.0)
            .replace("${user_type}", "offline")
            .replace("${user_properties}", "{}")
    }

    fn get_log_masks(&self) -> Vec<String> {
        vec![]
    }
}
