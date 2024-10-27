use std::{ffi::OsString, fmt::Display, marker::PhantomData};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use base64::prelude::*;

use crate::{minecraft::{login::{Account, AccountConstructor}, version::MinecraftInstallation}, utils::{check_hash, download, BetterPath}, LauncherContext};

use super::{YggdrasilAccount, YggdrasilUserData};

#[derive(Serialize, Deserialize)]
struct AuthlibInjectorAccount {
    #[serde(flatten)]
    data: Option<YggdrasilUserData>
}

impl Display for AuthlibInjectorAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = &self.get_data() {
            write!(f, "{} ({})", v.name, v.server_name)
        } else {
            write!(f, "{} {}", t!("accounts.uninitialized"), t!("accounts.authlib_injector.name"))
        }
    }
}

pub(crate) struct AuthlibInjectorAccountConstructor;

impl AccountConstructor for AuthlibInjectorAccountConstructor {
    fn new(&self) -> Box<dyn Account> {
        Box::new(AuthlibInjectorAccount {
            data: None
        })
    }

    fn deserialize(&self, de: &mut dyn erased_serde::Deserializer) -> Option<Box<dyn Account>> {
        Some(Box::new(erased_serde::deserialize::<AuthlibInjectorAccount>(de).ok()?))
    }
}

#[async_trait]
impl YggdrasilAccount for AuthlibInjectorAccount {
    fn is_initialized(&self) -> bool {
        self.data.is_some()
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

    async fn ask_api_url(&mut self, launcher: &LauncherContext) -> String {
        let api_url = launcher.ui.ask_user_one(&t!("accounts.authlib_injector.apiurl"), None).await; // TODO: i18n
        let res = launcher.http_client.get(&api_url).send().await;
        if res.is_err() {
            return api_url;
        }
        if let Ok(s) = res.unwrap().headers()["x-authlib-injector-api-location"].to_str() {
            return s.to_string();
        }
        return api_url;
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
        let content = launcher.http_client.get(self.data.as_ref().unwrap().api_url.clone()).send().await?.bytes().await?;
        Ok(vec![
            OsString::from(format!("-javaagent:./authlib-injector-latest.jar={}", self.data.as_ref().unwrap().api_url)),
            OsString::from(format!("-Dauthlibinjector.yggdrasil.prefetched={}", BASE64_STANDARD.encode(content)))
        ])
    }
}
