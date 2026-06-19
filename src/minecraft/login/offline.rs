use std::{collections::HashMap, ffi::OsString, fmt::Display};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use uuid::{Builder, Uuid};

use crate::{LauncherContext, minecraft::{login::AccountTrait, version::MinecraftInstallation}, utils::BetterPath};



#[derive(Serialize, Deserialize)]
pub struct OfflineAccount(String);

impl Display for OfflineAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.0, t!("accounts.offline.name"))
    }
}

impl AccountTrait for OfflineAccount {

    async fn check(&mut self, _: &LauncherContext) -> bool {
        true
    }

    fn get_uuid(&self) -> Uuid {
        Builder::from_md5_bytes(md5::compute(self.0.clone()).0).into_uuid()
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
        map.insert("${auth_player_name}".to_string(), self.0.clone());
        map.insert("${user_type}".to_string(), "offline".to_string());
        map.insert("${user_properties}".to_string(), "{}".to_string());
        return map;
    }

    fn get_log_masks(&self) -> Vec<String> {
        vec![]
    }
}

async fn login(launcher: &LauncherContext) -> Result<OfflineAccount> {
    Ok(OfflineAccount(launcher.ui.ask_user_one(&t!("accounts.offline.username"), None).await.ok_or(anyhow!("User cancelled."))?))
}
