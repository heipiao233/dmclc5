//! The authentication method that Mojang uses before the migration. Still used by third party.

pub(crate) mod mul;
pub(crate) mod ali;

use std::{ffi::OsString, fmt::Display, path::Path};

use enum_dispatch::enum_dispatch;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{LauncherConfig, minecraft::login::{Account, AccountError, AccountTrait, Result}};
pub use ali::AuthlibInjectorAccount;
pub use mul::MinecraftUniversalLoginAccount;


/// Stored user data for a [YggdrasilAccount].
#[derive(Serialize, Deserialize, Clone)]
pub struct YggdrasilUserData {
    api_url: String,
    server_name: String,
    client_token: String,
    name: String,
    uuid: Uuid,
    at: String
}

/// A Yggdrasil account profile, with dedicated game nickname, skin and cape.
#[derive(Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The UUID of this profile
    pub id: Uuid,
    /// The game nickname
    pub name: String
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
#[derive(Serialize, Deserialize, Clone)]
pub struct YggdrasilAccount<A: YggdrasilAccountTrait> {
    data: YggdrasilUserData,
    extra: A
}

impl <A: YggdrasilAccountTrait> Display for YggdrasilAccount<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.data.name, self.data.server_name)
    }
}

/// The interface of [YggdrasilAccount]
#[enum_dispatch(YggdrasilAccount)]
#[allow(async_fn_in_trait)]
pub trait YggdrasilAccountTrait: Clone {
    /// Prepare for launch.
    async fn prepare_launch(&self, version_launch_dir: &Path, launcher: &LauncherConfig) -> Result<()>;
    /// Get additional JVM arguments.
    async fn get_launch_jvmargs(&self, data: &YggdrasilUserData, launcher: &LauncherConfig) -> Result<Vec<OsString>>;
}

impl <A: YggdrasilAccountTrait> AccountTrait for YggdrasilAccount<A>
where Account: From<Self> {
    async fn check(self, launcher: &LauncherConfig) -> Result<Account> {
        let data = &self.data;
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
        Ok(Self {
            data: YggdrasilUserData {
                at: res.access_token,
                client_token: res.client_token,
                ..self.data
            },
            extra: self.extra
        }.into())
    }

    fn get_uuid(&self) -> &Uuid {
        &self.data.uuid
    }

    async fn prepare_launch(&self, version_launch_dir: &Path, launcher: &LauncherConfig) -> Result<()> {
        self.extra.prepare_launch(version_launch_dir, launcher).await
    }

    async fn get_launch_jvmargs(&self, launcher: &LauncherConfig) -> Result<Vec<OsString>> {
        self.extra.get_launch_jvmargs(&self.data, launcher).await
    }

    fn replace_launch_game_arg(&self, arg: &str) -> String {
        let data = &self.data;
        arg.replace("${auth_access_token}", &data.at)
            .replace("${auth_session}", &data.at)
            .replace("${auth_player_name}", &data.name)
            .replace("${user_type}", "msa")
            .replace("${user_properties}", "{}")
    }

    fn get_log_masks(&self) -> Vec<String> {
        vec![self.data.at.clone(), self.data.client_token.clone()]
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
pub async fn auth(launcher: &LauncherConfig, api_url: String, username: String, password: String) -> Result<(YggdrasilAuthInfo, Vec<Profile>)> {
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
        return Err(AccountError::YggdrasilError(auth_res.status()));
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
