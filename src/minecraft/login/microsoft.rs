//! The authentication method that Mojang uses now.

use std::{collections::HashMap, ffi::OsString, fmt::Display};

use anyhow::{anyhow, Ok, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use tokio::time;

use crate::{minecraft::version::MinecraftInstallation, utils::BetterPath, LauncherContext};

use super::{Account, AccountConstructor};

const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Deserialize)]
struct Step1Response {
    access_token: String,
    refresh_token: String
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Step1ResponseLoop {
    Successful(Step1Response),
    Error {
        error: String
    }
}

#[derive(Deserialize)]
struct DeviceAuthorizationResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: usize,
    expires_in: usize
}

#[derive(Serialize, Deserialize)]
pub(crate) struct MicrosoftAccount {
    #[serde(flatten)]
    data: Option<MicrosoftAccountData>
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct MicrosoftAccountData {
    refresh_token: String,
    name: String,
    uuid: Uuid,
    at: String
}

pub(crate) struct MicrosoftAccountConstructor;

impl AccountConstructor for MicrosoftAccountConstructor {
    fn new(&self) -> Box<dyn Account> {
        Box::new(MicrosoftAccount{
            data: None
        })
    }

    fn deserialize(&self, de: &mut dyn erased_serde::Deserializer) -> Option<Box<dyn Account>> {
        Some(Box::new(erased_serde::deserialize::<MicrosoftAccount>(de).ok()?))
    }
}

impl MicrosoftAccount {
    async fn refresh(&mut self, launcher: &LauncherContext) -> Result<()> {
        let at: Value = launcher.http_client.post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[("client_id", launcher.ms_client_id.as_str()), ("grant_type", "refresh_token"), ("refresh_token", &self.data.as_ref().unwrap().refresh_token)])
            .send().await?.json().await?;
        let refresh_token = at["refresh_token"].as_str().clone().unwrap().to_string();
        let next = self.next_steps(at["access_token"].as_str().unwrap(), &launcher).await?;
        self.data = Some(MicrosoftAccountData {
            refresh_token,
            name: next.2,
            uuid: next.0,
            at: next.1
        });
        Ok(())
    }

    async fn next_steps(&self, access_token: &str, launcher: &LauncherContext) -> Result<(Uuid, String, String)> {
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

    async fn loop_for_auth(&self, flow: DeviceAuthorizationResponse, launcher: &LauncherContext) -> Result<Step1Response> {
        let start_time = time::Instant::now();
        let mut interval = time::Duration::from_secs(flow.interval as u64);
        let expires_in = time::Duration::from_secs(flow.expires_in as u64);
        loop {
            time::sleep(interval).await;
            let estimated = time::Instant::now() - start_time;
            if estimated >= expires_in {
                return Result::Err(anyhow!(t!("accounts.microsoft.timeout")).into());
            }
            let dev_flow_res: Step1ResponseLoop = launcher.http_client.post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("code", &flow.device_code),
                    ("client_id", &launcher.ms_client_id)
                ]).send().await?.json().await?;
            if let Step1ResponseLoop::Error { error } = dev_flow_res {
                match error.as_str() {
                    "expired_token" => return Result::Err(anyhow!(t!("accounts.microsoft.timeout")).into()),
                    "authorization_declined" => return Result::Err(anyhow!(t!("accounts.microsoft.canceled")).into()),
                    "slow_down" => interval += time::Duration::from_secs(5),
                    "authorization_pending" => continue,
                    _ => return Result::Err(anyhow!("Token flow error: {error}"))
                }
            } else if let Step1ResponseLoop::Successful(res) = dev_flow_res {
                return Ok(res);
            }
        }
    }
}

impl Display for MicrosoftAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = &self.data {
            write!(f, "{} ({})", v.name, t!("accounts.microsoft.name"))
        } else {
            write!(f, "{} {}", t!("accounts.uninitialized"), t!("accounts.microsoft.name"))
        }
    }
}

#[async_trait]
impl Account for MicrosoftAccount {
    fn is_initialized(&self) -> bool {
        self.data.is_some()
    }

    async fn check(&mut self, launcher: &LauncherContext) -> bool {
        self.is_initialized() && self.refresh(&launcher).await.is_ok()
    }

    async fn login(&mut self, launcher: &LauncherContext) -> Result<()> {
        let dev_flow: DeviceAuthorizationResponse = launcher.http_client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
            .form(&[("client_id", launcher.ms_client_id.as_str()), ("scope", SCOPE)])
            .send().await?.json().await?;
        launcher.ui.info(&t!("accounts.microsoft.message", url = dev_flow.verification_uri, code = dev_flow.user_code), "MSA Login").await; // TODO: i18n
        let _ = open::that(&dev_flow.verification_uri);
        let dev_flow_res = self.loop_for_auth(dev_flow, &launcher).await?;
        let refresh_token = dev_flow_res.refresh_token.clone();
        let next = self.next_steps(&dev_flow_res.access_token, &launcher).await?;
        self.data = Some(MicrosoftAccountData {
            refresh_token,
            name: next.2,
            uuid: next.0,
            at: next.1
        });

        Ok(())
    }

    fn get_uuid(&self) -> Uuid {
        self.data.clone().unwrap().uuid
    }

    async fn prepare_launch(&self, _: &BetterPath, _: &LauncherContext) -> Result<()> {
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _: &MinecraftInstallation, _: &LauncherContext) -> Result<Vec<OsString>> {
        Ok(vec![])
    }

    async fn get_launch_game_args(&mut self, launcher: &LauncherContext) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let _ = self.refresh(&launcher).await;
        let at = self.data.clone().unwrap().at;
        map.insert("${auth_access_token}".to_string(), at.clone());
        map.insert("${auth_session}".to_string(), at);
        map.insert("${auth_player_name}".to_string(), self.data.clone().unwrap().name);
        map.insert("${user_type}".to_string(), "msa".to_string());
        map.insert("${user_properties}".to_string(), "{}".to_string());
        return map;
    }

    fn get_log_masks(&self) -> Vec<String> {
        if let Some(v) = &self.data {
            vec![v.at.clone()]
        } else {
            vec![]
        }
    }
}
