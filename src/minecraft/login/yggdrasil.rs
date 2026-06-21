//! The authentication method that Mojang uses before the migration. Still used by third party.

pub(crate) mod mul;
pub(crate) mod ali;

use std::{collections::HashMap, ffi::OsString, fmt::Display};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{LauncherContext, minecraft::{login::AccountTrait, version::MinecraftInstallation}, utils::BetterPath};
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

#[derive(Deserialize, Serialize)]
struct Profile {
    id: Uuid,
    name: String
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    access_token: String,
    client_token: String,
    available_profiles: Vec<Profile>
}

/// A kind of [Account] that Mojang uses before the migration. Used by some servers.
/// With some mod (mostly based on Java Agent) they inject and modify the auth server to third party ones.
/// This doesn't provide proof of purchase nowadays.
#[enum_dispatch]
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

#[async_trait]
#[enum_dispatch(YggdrasilAccount)]
trait YggdrasilAccountTrait: Send + Sync + Display {
    /// Get the [YggdrasilUserData].
    fn get_data(&self) -> &YggdrasilUserData;
    /// Get the API url.
    async fn get_api_url(&mut self) -> Result<String> {
        Ok(self.get_data().api_url.clone())
    }
    /// Prepare for launch.
    async fn prepare_launch(&self, version_launch_dir: &BetterPath, launcher: &LauncherContext) -> Result<()>;
    /// Get additional JVM arguments.
    async fn get_launch_jvmargs(&self, mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>>;
}

#[async_trait]
impl AccountTrait for YggdrasilAccount {

    async fn check(&mut self, launcher: &LauncherContext) -> bool {
        let api_url = self.get_api_url().await;
        if api_url.is_err() {
            return false;
        }
        let api_url = api_url.unwrap();
        let http = &(launcher.http_client);
        let data = self.get_data();
        let req: Value = json!({
            "accessToken": data.at,
            "clientToken": data.client_token
        });
        let res = http.post(format!("{api_url}/authserver/validate"))
            .json(&req)
            .send().await;
        res.is_ok() && res.unwrap().status() == StatusCode::NO_CONTENT
    }

    fn get_uuid(&self) -> Uuid {
        self.get_data().uuid
    }

    async fn prepare_launch(&self, version_launch_dir: &BetterPath, launcher: &LauncherContext) -> Result<()> {
        YggdrasilAccountTrait::prepare_launch(self, version_launch_dir, launcher).await
    }

    async fn get_launch_jvmargs(&self, mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>> {
        YggdrasilAccountTrait::get_launch_jvmargs(self, mc, launcher).await
    }

    async fn get_launch_game_args(&mut self, _: &LauncherContext) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let data = self.get_data();
        let at = data.at.clone();
        map.insert("${auth_access_token}".to_string(), at.clone());
        map.insert("${auth_session}".to_string(), at);
        map.insert("${auth_player_name}".to_string(), data.name.clone());
        map.insert("${user_type}".to_string(), "mojang".to_string());
        map.insert("${user_properties}".to_string(), "{}".to_string());
        return map;
    }

    fn get_log_masks(&self) -> Vec<String> {
        vec![self.get_data().at.clone(), self.get_data().client_token.clone()]
    }
}

async fn login(launcher: &LauncherContext, api_url: String, username: String, password: String) -> Result<YggdrasilUserData> {
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
    let profile_id = launcher.ui.ask_user_choose(auth_res.available_profiles.iter().map(|i|i.name.as_str()).collect(), "Please select profile").await.ok_or(anyhow!("User cancelled"))?; // TODO: i18n
    let profile = &auth_res.available_profiles[profile_id];
    Ok(YggdrasilUserData {
        api_url,
        server_name,
        client_token: auth_res.client_token,
        name: profile.name.clone(),
        uuid: profile.id,
        at: auth_res.access_token
    })
}
