//! The authentication method that Mojang uses before the migration.

pub(crate) mod mul;
pub(crate) mod ali;

use std::{collections::HashMap, ffi::OsString, fmt::Display};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{minecraft::version::MinecraftInstallation, utils::BetterPath, LauncherContext};

use super::Account;

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

/// A kind of [Account] that Mojang uses before the migration.
#[async_trait]
pub trait YggdrasilAccount: Send + Sync + Display {
    /// Check if it's initialized.
    fn is_initialized(&self) -> bool;
    /// Get the [YggdrasilUserData].
    fn get_data(&self) -> &Option<YggdrasilUserData>;
    /// Get [YggdrasilUserData] (mutable).
    fn get_data_mut(&mut self) -> &mut Option<YggdrasilUserData>;
    /// Set [YggdrasilUserData].
    fn set_data(&mut self, data: YggdrasilUserData);
    /// Ask user the API url.
    async fn ask_api_url(&mut self, launcher: &LauncherContext) -> Result<String>;
    /// Get the API url.
    async fn get_api_url(&mut self, launcher: &LauncherContext) -> Result<String> {
        if let Some(data) = self.get_data() {
            Ok(data.api_url.clone())
        } else {
            self.ask_api_url(&launcher).await
        }
    }
    /// Prepare for launch.
    async fn prepare_launch(&self, version_launch_dir: &BetterPath, launcher: &LauncherContext) -> Result<()>;
    /// Get additional JVM arguments.
    async fn get_launch_jvmargs(&self, mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>>;
}

#[async_trait]
impl <T: YggdrasilAccount> Account for T {

    fn is_initialized(&self) -> bool {
        self.is_initialized()
    }

    async fn check(&mut self, launcher: &LauncherContext) -> bool {
        if !self.is_initialized() {
            return false;
        }
        let api_url = self.get_api_url(&launcher).await;
        if let Err(_) = api_url {
            return false;
        }
        let api_url = api_url.unwrap();
        let http = &(launcher.http_client);
        let data = self.get_data().as_ref().unwrap();
        let req: Value = json!({
            "accessToken": data.at,
            "clientToken": data.client_token
        });
        let res = http.post(format!("{api_url}/authserver/validate"))
            .json(&req)
            .send().await;
        res.is_ok() && res.unwrap().status() == StatusCode::NO_CONTENT
    }

    async fn login(&mut self, launcher: &LauncherContext) -> Result<()> {
        let content = launcher.ui.ask_user(vec![
            ("username", "Username"),
            ("password", "Password")
        ], None).await.ok_or(anyhow!("User cancelled"))?; // TODO: i18n
        let api_url = self.get_api_url(&launcher).await?;
        let http = &launcher.http_client;

        let meta: Value = http.get(&api_url).send().await?.json().await?;
        let server_name = meta["meta"]["serverName"].as_str().unwrap().to_string();

        let auth_req = json!({
            "username": content["username"],
            "password": content["password"],
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
        self.set_data(YggdrasilUserData {
            api_url,
            server_name,
            client_token: auth_res.client_token,
            name: profile.name.clone(),
            uuid: profile.id.clone(),
            at: auth_res.access_token
        });
        Ok(())
    }

    fn get_uuid(&self) -> Uuid {
        self.get_data().as_ref().unwrap().uuid
    }

    async fn prepare_launch(&self, version_launch_dir: &BetterPath, launcher: &LauncherContext) -> Result<()> {
        YggdrasilAccount::prepare_launch(self, version_launch_dir, &launcher).await
    }

    async fn get_launch_jvmargs(&self, mc: &MinecraftInstallation, launcher: &LauncherContext) -> Result<Vec<OsString>> {
        YggdrasilAccount::get_launch_jvmargs(self, mc, &launcher).await
    }

    async fn get_launch_game_args(&mut self, _: &LauncherContext) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let data = self.get_data().as_ref().unwrap();
        let at = data.at.clone();
        map.insert("${auth_access_token}".to_string(), at.clone());
        map.insert("${auth_session}".to_string(), at);
        map.insert("${auth_player_name}".to_string(), data.name.clone());
        map.insert("${user_type}".to_string(), "mojang".to_string());
        map.insert("${user_properties}".to_string(), "{}".to_string());
        return map;
    }

    fn get_log_masks(&self) -> Vec<String> {
        if let Some(data) = self.get_data() {
            vec![data.at.clone(), data.client_token.clone()]
        } else {
            vec![]
        }
    }
}
