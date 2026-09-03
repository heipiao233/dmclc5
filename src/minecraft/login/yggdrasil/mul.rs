use std::{ffi::OsString, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{LauncherConfig, minecraft::{login::yggdrasil::{self, Profile, YggdrasilAccount, YggdrasilAccountTrait, YggdrasilAuthInfo}}, utils::download};

use super::YggdrasilUserData;

/// An account with nide8.
#[derive(Serialize, Deserialize)]
pub struct MinecraftUniversalLoginAccount {
    server_id: String
}

impl YggdrasilAccountTrait for MinecraftUniversalLoginAccount {
    async fn prepare_launch(&self, version_launch_dir: &Path, _: &LauncherConfig) -> Result<()> {
        let path = version_launch_dir.join("nide8auth.jar");
        if std::fs::metadata(&path).is_err() {
            download("https://login.mc-user.com:233/index/jar", &path).await?;
        }
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _data: &YggdrasilUserData, _: &LauncherConfig) -> Result<Vec<OsString>> {
        Ok(vec![OsString::from(format!("-javaagent:./nide8auth.jar={}", self.server_id)), OsString::from("-Dnide8auth.client=true")])
    }
}

impl MinecraftUniversalLoginAccount {
    /// Login to MinecraftUniversalLogin, return tokens and profiles.
    pub async fn auth(launcher: &LauncherConfig, server_id: String, username: String, password: String) -> Result<(YggdrasilAuthInfo, Vec<Profile>)> {
        yggdrasil::auth(launcher, format!("https://auth.mc-user.com:233/{server_id}"), username, password).await
    }

    /// Create a [MinecraftUniversalLoginAccount] with authenicated tokens and selected profile from [Self::auth]
    pub fn new(auth_info: YggdrasilAuthInfo, profile: Profile, server_id: String) -> YggdrasilAccount<Self> {
        YggdrasilAccount(YggdrasilUserData::new(auth_info, profile), Self {
            server_id
        })
    }
}
