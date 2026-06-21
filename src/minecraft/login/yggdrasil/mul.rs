use std::{ffi::OsString, fmt::Display};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{LauncherContext, minecraft::{login::yggdrasil::{self, YggdrasilAccountTrait}, version::MinecraftInstallation}, utils::{BetterPath, download}};

use super::YggdrasilUserData;

#[derive(Serialize, Deserialize)]
pub struct MinecraftUniversalLoginAccount {
    data: YggdrasilUserData,
    server_id: String
}

impl Display for MinecraftUniversalLoginAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.data.name, self.data.server_name)
    }
}

#[async_trait]
impl YggdrasilAccountTrait for MinecraftUniversalLoginAccount {
    fn get_data(&self) -> &YggdrasilUserData {
        &self.data
    }

    async fn prepare_launch(&self, version_launch_dir: &BetterPath, _: &LauncherContext) -> Result<()> {
        let path = version_launch_dir / "nide8auth.jar";
        if fs::metadata(&*path).await.is_err() {
            download("https://login.mc-user.com:233/index/jar", path.as_ref()).await?;
        }
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _mc: &MinecraftInstallation, _: &LauncherContext) -> Result<Vec<OsString>> {
        Ok(vec![OsString::from(format!("-javaagent:./nide8auth.jar={}", self.server_id)), OsString::from("-Dnide8auth.client=true")])
    }
}

impl MinecraftUniversalLoginAccount {
    pub async fn login(launcher: &LauncherContext, server_id: String, username: String, password: String) -> Result<Self> {
        Ok(Self {
            data: yggdrasil::login(launcher, format!("https://auth.mc-user.com:233/{server_id}"), username, password).await?,
            server_id
        })
    }
}
