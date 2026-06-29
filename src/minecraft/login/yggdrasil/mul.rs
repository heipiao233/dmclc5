use std::{ffi::OsString, fmt::Display};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{LauncherContext, minecraft::{login::yggdrasil::{self, Profile, YggdrasilAccountTrait, YggdrasilAuthInfo}, version::MinecraftInstallation}, utils::{BetterPathBuf, download}};

use super::YggdrasilUserData;

/// An account with nide8.
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

impl YggdrasilAccountTrait for MinecraftUniversalLoginAccount {
    fn get_data(&self) -> &YggdrasilUserData {
        &self.data
    }

    fn with_cred(self, client_token: String, access_token: String) -> Self {
        Self {
            data: YggdrasilUserData {
                client_token,
                at: access_token,
                ..self.data
            },
            ..self
        }
    }

    async fn prepare_launch(&self, version_launch_dir: &BetterPathBuf, _: &LauncherContext) -> Result<()> {
        let path = version_launch_dir.clone() / "nide8auth.jar";
        if fs::metadata(&path).await.is_err() {
            download("https://login.mc-user.com:233/index/jar", &path).await?;
        }
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _mc: &MinecraftInstallation, _: &LauncherContext) -> Result<Vec<OsString>> {
        Ok(vec![OsString::from(format!("-javaagent:./nide8auth.jar={}", self.server_id)), OsString::from("-Dnide8auth.client=true")])
    }
}

impl MinecraftUniversalLoginAccount {
    /// Login to MinecraftUniversalLogin, return tokens and profiles.
    pub async fn auth(launcher: &LauncherContext, server_id: String, username: String, password: String) -> Result<(YggdrasilAuthInfo, Vec<Profile>)> {
        yggdrasil::auth(launcher, format!("https://auth.mc-user.com:233/{server_id}"), username, password).await
    }

    /// Create a [MinecraftUniversalLoginAccount] with authenicated tokens and selected profile from [Self::auth]
    pub fn new(auth_info: YggdrasilAuthInfo, profile: Profile, server_id: String) -> MinecraftUniversalLoginAccount {
        Self { data: YggdrasilUserData::new(auth_info, profile), server_id }
    }
}
