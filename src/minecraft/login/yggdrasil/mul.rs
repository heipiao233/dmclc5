use std::{ffi::OsString, fmt::Display};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{LauncherContext, minecraft::{login::{yggdrasil::YggdrasilAccountTrait}, version::MinecraftInstallation}, utils::{BetterPath, download}};

use super::{YggdrasilAccount, YggdrasilUserData};

#[derive(Serialize, Deserialize)]
pub(crate) struct MinecraftUniversalLoginAccount {
    data: YggdrasilUserData,
    server_id: String
}

impl Display for MinecraftUniversalLoginAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.data.name, self.data.server_name)
    }
}

impl YggdrasilAccountTrait for MinecraftUniversalLoginAccount {

    fn get_data(&self) -> &YggdrasilUserData {
        &self.data
    }

    fn get_data_mut(&mut self) -> &mut YggdrasilUserData {
        &mut self.data
    }

    fn set_data(&mut self, data: YggdrasilUserData) {
        self.data = data;
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
