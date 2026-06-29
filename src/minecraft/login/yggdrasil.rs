//! The authentication method that Mojang uses before the migration. Still used by third party.

pub(crate) mod mul;
pub(crate) mod ali;

use std::{collections::HashMap, ffi::OsString, fmt::Display};

use anyhow::{Result, anyhow};
use enum_dispatch::enum_dispatch;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{LauncherContext, minecraft::{login::{Account, AccountTrait}, version::MinecraftInstallation}, utils::BetterPathBuf};
pub use ali::AuthlibInjectorAccount;
pub use mul::MinecraftUniversalLoginAccount;


/// Stored user data for a [YggdrasilAccount].
#[derive(Serialize, Deserialize)]
pub struct YggdrasilUserData {
    api_url: String,
    server_name: String,
    client_token: String,
    name: String,
    uuid: Uuid,
    at: String
}

/// A Yggdrasil account profile, with dedicated game nickname, skin and cape.
#[derive(Deserialize, Serialize)]
pub struct Profile {
    id: Uuid,
    name: String
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    access_token: String,
    client_token: String,
    pub available_profiles: Vec<Profile>
}

/// A kind of [Account] that Mojang uses before the migration. Used by some servers.
/// With some mod (mostly based on Java Agent) they inject and modify the auth server to third party ones.
/// This doesn't provide proof of purchase nowadays.
#[enum_dispatch]
#[derive(Serialize, Deserialize)]
pub enum YggdrasilAccount {
    /// The most widely used alternative Yggdrasil injection.
    /// See its source code in https://github.com/yushijinhun/authlib-injector
    AuthlibInjectorAccount,
    /// So-called ["Minecraft 统一通行证"](https://login.mc-user.com:233/) or "nide8". Provide per-server authenication.
    /// This injects a third party proprietary software provided by them to Minecraft.
    MinecraftUniversalLoginAccount
}

impl Display for YggdrasilAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthlibInjectorAccount(account) => account.fmt(f),
            Self::MinecraftUniversalLoginAccount(account) => account.fmt(f)
        }
    }
}

#[enum_dispatch(YggdrasilAccount)]
trait YggdrasilAccountTrait: Display {
    /// Get the [YggdrasilUserData].
    fn get_data(&self) -> &YggdrasilUserData;
    /// Set credentials.
    fn with_cred(self, client_token: String, access_token: String) -> Self;
    /// Prepare for launch.
    async fn prepare_launch(&self, version_launch_dir: &BetterPathBuf, launcher: &LauncherContext) -> Result<()>;
    /// Get additional JVM arguments.
    async fn get_launch_jvmargs(&self, mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>>;
}

impl AccountTrait for YggdrasilAccount {
    async fn check(self, launcher: &LauncherContext) -> Result<Account> {
        let data = self.get_data();
        let req: Value = json!({
            "accessToken": data.at,
            "clientToken": data.client_token
        });
        let res = launcher.http_client.post(format!("{}/authserver/validate", data.api_url))
            .json(&req)
            .send().await;
        if let Ok(res) = res && res.status() == StatusCode::NO_CONTENT {
            return Ok(self.into());
        }
        let res: AuthResponse = launcher.http_client.post(format!("{}/authserver/refresh", data.api_url))
            .json(&req)
            .send().await?.json().await?;
        Ok(self.with_cred(res.client_token, res.access_token).into())
    }

    fn get_uuid(&self) -> Uuid {
        self.get_data().uuid
    }

    async fn prepare_launch(&self, version_launch_dir: &BetterPathBuf, launcher: &LauncherContext) -> Result<()> {
        YggdrasilAccountTrait::prepare_launch(self, version_launch_dir, launcher).await
    }

    async fn get_launch_jvmargs(&self, mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>> {
        YggdrasilAccountTrait::get_launch_jvmargs(self, mc, launcher).await
    }

    fn replace_launch_game_arg(&self, arg: &String) -> String {
        let data = self.get_data();
        arg.replace("${auth_access_token}", &data.at)
            .replace("${auth_session}", &data.at)
            .replace("${auth_player_name}", &data.name)
            .replace("${user_type}", "msa")
            .replace("${user_properties}", "{}")
    }

    fn get_log_masks(&self) -> Vec<String> {
        vec![self.get_data().at.clone(), self.get_data().client_token.clone()]
    }
}

/// Yggdrasil authenication info, including tokens.
pub struct YggdrasilAuthInfo {
    access_token: String,
    client_token: String,
    api_url: String,
    server_name: String
}

/// Login to a Yggdrasil server, return tokens and profiles.
pub async fn auth(launcher: &LauncherContext, api_url: String, username: String, password: String) -> Result<(YggdrasilAuthInfo, Vec<Profile>)> {
    let http = &launcher.http_client;

    let meta: Value = http.get(&api_url).send().await?.json().await?;
    let server_name = meta["meta"]["serverName"].as_str().unwrap().to_string();

    let auth_req = json!({
        "username": username,
        "password": password,
        "requestUser": true,
        "agent": {
            "name": "Minecraft",
            "version": 1
        }
    });
    let auth_res = http.post(format!("{api_url}/authserver/authenticate"))
        .json(&auth_req)
        .send().await?;
    if auth_res.status().is_client_error() {
        return Err(anyhow!("Yggdrasil auth returned error code {}", auth_res.status())); // TODO: i18n
    }
    let auth_res: AuthResponse = auth_res.json().await?;
    Ok((YggdrasilAuthInfo {
        access_token: auth_res.access_token,
        client_token: auth_res.client_token,
        api_url,
        server_name
    }, auth_res.available_profiles))
}

impl YggdrasilUserData {
    /// Create a [YggdrasilUserData] with authenicated tokens and selected profile from [yggdrasil::auth]
    pub fn new(auth_info: YggdrasilAuthInfo, profile: Profile) -> YggdrasilUserData {
        Self {
            api_url: auth_info.api_url,
            server_name: auth_info.server_name,
            client_token: auth_info.client_token,
            name: profile.name,
            uuid: profile.id,
            at: auth_info.access_token
        }
    }
}
