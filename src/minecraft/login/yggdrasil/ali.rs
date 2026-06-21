use std::{ffi::OsString, fmt::Display, marker::PhantomData};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use base64::prelude::*;

use crate::{LauncherContext, minecraft::{login::yggdrasil::{self, YggdrasilAccountTrait}, version::MinecraftInstallation}, utils::{BetterPath, check_hash, download}};

use super::YggdrasilUserData;

#[derive(Serialize, Deserialize)]
pub struct AuthlibInjectorAccount {
    #[serde(flatten)]
    data: YggdrasilUserData
}

impl Display for AuthlibInjectorAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.data.name, self.data.server_name)
    }
}

#[async_trait]
impl YggdrasilAccountTrait for AuthlibInjectorAccount {

    fn get_data(&self) -> &YggdrasilUserData {
        &self.data
    }

    async fn prepare_launch(&self, version_launch_dir: &BetterPath, launcher: &LauncherContext) -> Result<()> {
        let path = version_launch_dir / "authlib-injector-latest.jar";
        let release_info: Value = launcher.http_client
            .get("https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json")
            .send().await?.json().await?;
        if check_hash(&path, release_info["checksums"]["sha256"].as_str().ok_or(anyhow!("No sha256 in checksums."))?, 0, PhantomData::<Sha256>).await { // TODO: i18n
            return Ok(());
        }
        download(release_info["download_url"].as_str().ok_or(anyhow!("Invaild download URL"))?, &path).await?; // TODO: i18n
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>> {
        let content = launcher.http_client.get(self.data.api_url.clone()).send().await?.bytes().await?;
        Ok(vec![
            OsString::from(format!("-javaagent:./authlib-injector-latest.jar={}", self.data.api_url)),
            OsString::from(format!("-Dauthlibinjector.yggdrasil.prefetched={}", BASE64_STANDARD.encode(content)))
        ])
    }
}

impl AuthlibInjectorAccount {
    pub async fn login(launcher: &LauncherContext, api_url: String, username: String, password: String) -> Result<Self> {
        Ok(Self {
            data: yggdrasil::login(launcher, api_url, username, password).await?
        })
    }
}
