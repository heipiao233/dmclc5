use std::{ffi::OsString, fmt::Display};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{minecraft::{login::{Account, AccountConstructor}, version::MinecraftInstallation}, utils::{download, BetterPath}, LauncherContext};

use super::{YggdrasilAccount, YggdrasilUserData};

#[derive(Serialize, Deserialize)]
struct MinecraftUniversalLoginAccount {
    data: Option<YggdrasilUserData>,
    server_id: Option<String>
}

impl Display for MinecraftUniversalLoginAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = &self.get_data() {
            write!(f, "{} ({})", v.name, v.server_name)
        } else {
            write!(f, "{} {}", t!("accounts.uninitialized"), t!("accounts.minecraft_universal_login.name"))
        }
    }
}

pub(crate) struct MinecraftUniversalLoginAccountConstructor;

impl AccountConstructor for MinecraftUniversalLoginAccountConstructor {
    fn new(&self) -> Box<dyn Account> {
        Box::new(MinecraftUniversalLoginAccount {
            data: None,
            server_id: None
        })
    }

    fn deserialize(&self, de: &mut dyn erased_serde::Deserializer) -> Option<Box<dyn Account>> {
        Some(Box::new(erased_serde::deserialize::<MinecraftUniversalLoginAccount>(de).ok()?))
    }
}

#[async_trait]
impl YggdrasilAccount for MinecraftUniversalLoginAccount {

    fn is_initialized(&self) -> bool {
        self.data.is_some() && self.server_id.is_some()
    }

    fn get_data(&self) -> &Option<YggdrasilUserData> {
        &self.data
    }

    fn get_data_mut(&mut self) -> &mut Option<YggdrasilUserData> {
        &mut self.data
    }

    fn set_data(&mut self, data: YggdrasilUserData) {
        self.data = Some(data);
    }

    async fn ask_api_url(&mut self, launcher: &LauncherContext) -> Result<String> {
        if self.server_id.is_none() {
            self.server_id = Some(launcher.ui.ask_user_one(&t!("accounts.minecraft_universal_login.serverID"), None).await.ok_or(anyhow!("User cancelled"))?);
        }
        Ok(format!("https://auth.mc-user.com:233/{}", self.server_id.as_ref().unwrap()))
    }

    async fn prepare_launch(&self, version_launch_dir: &BetterPath, _: &LauncherContext) -> Result<()> {
        let path = version_launch_dir / "nide8auth.jar";
        if fs::metadata(&*path).await.is_err() {
            download("https://login.mc-user.com:233/index/jar", path.as_ref()).await?;
        }
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _mc: &MinecraftInstallation, _: &LauncherContext) -> Result<Vec<OsString>> {
        Ok(vec![OsString::from(format!("-javaagent:./nide8auth.jar={}", self.server_id.as_ref().unwrap())), OsString::from("-Dnide8auth.client=true")])
    }
}
