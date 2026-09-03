//! The authentication method that Mojang uses now.

use std::{ffi::OsString, fmt::Display, path::Path};

use anyhow::{anyhow, Ok, Result};
use oauth2::{RefreshToken, Scope, TokenResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{LauncherConfig, minecraft::login::{Account, AccountTrait}};

/// An official account.
#[derive(Serialize, Deserialize)]
pub struct MicrosoftAccount {
    #[serde(skip)]
    refresh_token: Option<String>,
    name: String,
    uuid: Uuid,
    at: String
}

impl MicrosoftAccount {
    /// Begin with a creating of a [MicrosoftAccount]
    /// Show the [oauth2::StandardDeviceAuthorizationResponse] to your user.
    pub async fn start_auth(launcher: &LauncherConfig) -> Result<oauth2::StandardDeviceAuthorizationResponse> {
        Ok(launcher.oauth2.exchange_device_code()
            .add_scope(Scope::new("XboxLive.signin".to_string()))
            .add_scope(Scope::new("offline_access".to_string()))
            .request_async(&launcher.http_client).await?)
    }

    /// Begin with a creating of a [MicrosoftAccount]
    /// See: [Self::start_auth]
    pub async fn login(launcher: &LauncherConfig, device_auth: &oauth2::StandardDeviceAuthorizationResponse) -> Result<Self> {
        let dev_flow_res = launcher.oauth2.exchange_device_access_token(&device_auth)
            .request_async(&launcher.http_client, tokio::time::sleep, None).await?;
        let next = Self::next_steps(dev_flow_res.access_token().secret(), &launcher).await?;
        let refresh_token = dev_flow_res.refresh_token().map(RefreshToken::secret).cloned();
        if let Some(refresh_token) = &refresh_token {
            keyring::Entry::new(&format!("{} microsoft account", launcher.name), &next.0.to_string())?.set_password(refresh_token)?;
            println!("credential set for {}", next.0)
        }

        Ok(Self {
            refresh_token,
            name: next.2,
            uuid: next.0,
            at: next.1
        })
    }

    fn keyring_entry(&self, launcher: &LauncherConfig) -> Result<keyring::Entry> {
        Ok(keyring::Entry::new(&format!("{} microsoft account", launcher.name), &self.uuid.to_string())?)
    }

    async fn next_steps(access_token: &str, launcher: &LauncherConfig) -> Result<(Uuid, String, String)> {
        // XBL
        let xbl_req = json!(
            {
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": format!("d={access_token}")
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT"
            }
        );
        let xbl_res: Value = launcher.http_client
            .post("https://user.auth.xboxlive.com/user/authenticate")
            .json(&xbl_req)
            .send().await?.json().await?;
        let xbl_token = xbl_res["Token"].as_str()
            .ok_or(anyhow!("Token in XBL Response isn't a string"))?; // TODO: i18n
        let xbl_uhs = xbl_res["DisplayClaims"]["xui"][0]["uhs"].as_str()
            .ok_or(anyhow!("UHS in XBL Response isn't a string"))?; // TODO: i18n

        // XSTS
        let xsts_req = json!(
            {
                "Properties": {
                    "SandboxId": "RETAIL",
                    "UserTokens": [xbl_token]
                },
                "RelyingParty": "rp://api.minecraftservices.com/",
                "TokenType": "JWT"
            }
        );
        let xsts_res: Value = launcher.http_client
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .json(&xsts_req)
            .send().await?.json().await?;
        let xsts_token = xsts_res["Token"].as_str()
            .ok_or(anyhow!("Token in XSTS Response isn't a string"))?; // TODO: i18n

        // Minecraft Login
        let mclogin_req = json!(
            {
                "identityToken": format!("XBL3.0 x={xbl_uhs};{xsts_token}")
            }
        );
        let mclogin_res: Value = launcher.http_client
            .post("https://api.minecraftservices.com/authentication/login_with_xbox")
            .json(&mclogin_req)
            .send().await?.json().await?;
        let mclogin_at = mclogin_res["access_token"].as_str()
            .ok_or(anyhow!("Access token in MC Login Response isn't a string"))?; // TODO: i18n

        // We skip checking the ownership for XBox Game Pass...
        // MC Data
        let mcdata_res: Value = launcher.http_client.get("https://api.minecraftservices.com/minecraft/profile")
            .bearer_auth(mclogin_at)
            .send().await?.json().await?;
        if mcdata_res["error"].is_string() {
            return Err(anyhow!(t!("accounts.microsoft.no_minecraft_in_account"))); // TODO: i18n
        }
        let player_uuid = mcdata_res["id"].as_str()
            .ok_or(anyhow!("ID in MC Profile Response isn't a string"))?; // TODO: i18n
        let uuid = Uuid::parse_str(player_uuid)?;
        let at = mclogin_at.to_string();
        let name = mcdata_res["name"].as_str()
            .ok_or(anyhow!("Name in MC Profile Response isn't a string"))?.to_string(); // TODO: i18n

        Ok((uuid, at, name))
    }
}

impl Display for MicrosoftAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, t!("accounts.microsoft.name"))
    }
}

impl AccountTrait for MicrosoftAccount {
    async fn check(self, launcher: &LauncherConfig) -> Result<Account> {
        let refresh_token = if let Some(rt) = self.refresh_token {
            rt.clone()
        } else {
            self.keyring_entry(launcher)?.get_password()?
        };
        let at = launcher.oauth2.exchange_refresh_token(&RefreshToken::new(refresh_token))
            .add_scope(Scope::new("XboxLive.signin".to_string()))
            .add_scope(Scope::new("offline_access".to_string()))
            .request_async(&launcher.http_client).await?;
        let next = Self::next_steps(at.access_token().secret(), &launcher).await?;
        Ok(MicrosoftAccount {
            refresh_token: at.refresh_token().map(RefreshToken::secret).cloned(),
            name: next.2,
            uuid: next.0,
            at: next.1
        }.into())
    }

    fn get_uuid(&self) -> Uuid {
        self.uuid
    }

    async fn prepare_launch(&self, _: &Path, _: &LauncherConfig) -> Result<()> {
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _: &LauncherConfig) -> Result<Vec<OsString>> {
        Ok(vec![])
    }

    fn replace_launch_game_arg(&self, arg: &String) -> String {
        arg.replace("${auth_access_token}", &self.at)
            .replace("${auth_session}", &self.at)
            .replace("${auth_player_name}", &self.name)
            .replace("${user_type}", "msa")
            .replace("${user_properties}", "{}")
    }

    fn get_log_masks(&self) -> Vec<String> {
        vec![self.at.clone()]
    }
}
