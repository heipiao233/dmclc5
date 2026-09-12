//! The authentication method that Mojang uses now.

use std::{ffi::OsString, fmt::Display, path::Path};

use oauth2::{ClientId, DeviceAuthorizationUrl, DeviceCodeErrorResponseType, EndpointNotSet, EndpointSet, HttpClientError, RefreshToken, Scope, StandardErrorResponse, TokenResponse, TokenUrl, basic::{BasicClient, BasicErrorResponseType}};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{LauncherConfig, minecraft::login::{Account, AccountTrait, AccountError}};
pub use oauth2::StandardDeviceAuthorizationResponse;

/// An official account.
#[derive(Serialize, Deserialize, Clone)]
pub struct MicrosoftAccount {
    #[serde(skip)]
    refresh_token: Option<String>,
    name: String,
    uuid: Uuid,
    at: String
}

/// An error about Microsoft accounts.
#[derive(Debug, thiserror::Error)]
pub enum MSAError {
    /// OAuth error.
    #[error("OAuth Basic Error: {0}")]
    OAuthBasicError(#[from] oauth2::RequestTokenError<HttpClientError<reqwest::Error>, StandardErrorResponse<BasicErrorResponseType>>),
    /// OAuth error when using device flow.
    #[error("OAuth Device Flow Error: {0}")]
    OAuthDeviceError(#[from] oauth2::RequestTokenError<HttpClientError<reqwest::Error>, StandardErrorResponse<DeviceCodeErrorResponseType>>),
    /// Error when parsing an URL.
    #[error("OAuth URL Parse Error: {0}")]
    OAuthUrlParseError(#[from] oauth2::url::ParseError),
    /// Failure when reading/writting OS keyrings.
    #[error("Keyring Error: {0}")]
    KeyringError(#[from] keyring::Error),
    /// Network error.
    #[error("Network Error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    /// The player didn't purcase the game, and therefore cannot login.
    #[error("The player didn't purcase the game.")]
    GameNotPurcased
}

#[derive(Deserialize)]
struct XBoxDisplayClaimsXUI {
    uhs: String
}

#[derive(Deserialize)]
struct XboxDisplayClaims {
    xui: [XBoxDisplayClaimsXUI; 1]
}

#[derive(Deserialize)]
#[serde(rename_all="PascalCase")]
struct XboxResponse {
    token: String,
    display_claims: XboxDisplayClaims
}

#[derive(Deserialize)]
struct MinecraftAuthResponse {
    access_token: String
}

#[derive(Deserialize)]
struct MinecraftProfileResponse {
    id: Uuid,
    name: String
}

pub(crate) fn create_oauth2_client(ms_client_id: String) -> Result<BasicClient<EndpointNotSet, EndpointSet, EndpointNotSet, EndpointNotSet, EndpointSet>, MSAError> {
    Ok(oauth2::basic::BasicClient::new(ClientId::new(ms_client_id))
        .set_device_authorization_url(DeviceAuthorizationUrl::new("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode".to_string()).map_err(|e| MSAError::from(e))?)
        .set_token_uri(TokenUrl::new("https://login.microsoftonline.com/consumers/oauth2/v2.0/token".to_string()).map_err(|e| MSAError::from(e))?))
}

impl MicrosoftAccount {
    /// Begin with a creating of a [MicrosoftAccount]
    /// Show the [StandardDeviceAuthorizationResponse] to your user.
    pub async fn start_auth(launcher: &LauncherConfig) -> Result<StandardDeviceAuthorizationResponse, MSAError> {
        Ok(launcher.oauth2.exchange_device_code()
            .add_scope(Scope::new("XboxLive.signin".to_string()))
            .add_scope(Scope::new("offline_access".to_string()))
            .request_async(&launcher.http_client).await?)
    }

    /// Begin with a creating of a [MicrosoftAccount]
    /// See: [Self::start_auth]
    pub async fn login(launcher: &LauncherConfig, device_auth: &StandardDeviceAuthorizationResponse) -> Result<Self, MSAError> {
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

    fn keyring_entry(&self, launcher: &LauncherConfig) -> Result<keyring::Entry, MSAError> {
        Ok(keyring::Entry::new(&format!("{} microsoft account", launcher.name), &self.uuid.to_string())?)
    }

    async fn next_steps(access_token: &str, launcher: &LauncherConfig) -> Result<(Uuid, String, String), MSAError> {
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
        let xbl_res: XboxResponse = launcher.http_client
            .post("https://user.auth.xboxlive.com/user/authenticate")
            .json(&xbl_req)
            .send().await?.json().await?;
        let xbl_token = xbl_res.token;
        let xbl_uhs = &xbl_res.display_claims.xui[0].uhs;

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
        let xsts_res: XboxResponse = launcher.http_client
            .post("https://xsts.auth.xboxlive.com/xsts/authorize")
            .json(&xsts_req)
            .send().await?.json().await?;
        let xsts_token = xsts_res.token;

        // Minecraft Login
        let mclogin_req = json!(
            {
                "identityToken": format!("XBL3.0 x={xbl_uhs};{xsts_token}")
            }
        );
        let mclogin_res: MinecraftAuthResponse = launcher.http_client
            .post("https://api.minecraftservices.com/authentication/login_with_xbox")
            .json(&mclogin_req)
            .send().await?.json().await?;
        let mclogin_at = mclogin_res.access_token;

        // We skip checking the ownership for XBox Game Pass...
        // MC Data
        let mcdata_res = launcher.http_client.get("https://api.minecraftservices.com/minecraft/profile")
            .bearer_auth(&mclogin_at)
            .send().await?;
        if mcdata_res.status() == StatusCode::UNAUTHORIZED {
            return Err(MSAError::GameNotPurcased);
        }
        let mcdata_res: MinecraftProfileResponse = mcdata_res.json().await?;

        Ok((mcdata_res.id, mclogin_at, mcdata_res.name))
    }
}

impl Display for MicrosoftAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, t!("accounts.microsoft.name"))
    }
}

impl AccountTrait for MicrosoftAccount {
    async fn check(self, launcher: &LauncherConfig) -> Result<Account, AccountError> {
        let refresh_token = if let Some(rt) = self.refresh_token {
            rt.clone()
        } else {
            self.keyring_entry(launcher)?.get_password()
                .map_err(|e| MSAError::from(e))?
        };
        let at = launcher.oauth2.exchange_refresh_token(&RefreshToken::new(refresh_token))
            .add_scope(Scope::new("XboxLive.signin".to_string()))
            .add_scope(Scope::new("offline_access".to_string()))
            .request_async(&launcher.http_client).await
            .map_err(|e| MSAError::from(e))?;
        let next = Self::next_steps(at.access_token().secret(), &launcher).await?;
        Ok(MicrosoftAccount {
            refresh_token: at.refresh_token().map(RefreshToken::secret).cloned(),
            name: next.2,
            uuid: next.0,
            at: next.1
        }.into())
    }

    fn get_uuid(&self) -> &Uuid {
        &self.uuid
    }

    async fn prepare_launch(&self, _: &Path, _: &LauncherConfig) -> Result<(), AccountError> {
        Ok(())
    }

    async fn get_launch_jvmargs(&self, _: &LauncherConfig) -> Result<Vec<OsString>, AccountError> {
        Ok(vec![])
    }

    fn replace_launch_game_arg(&self, arg: &str) -> String {
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
