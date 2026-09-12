use std::{ffi::OsString, path::Path};

use serde::{Deserialize, Serialize};
use sha2::Sha256;
use base64::prelude::*;

use crate::{LauncherConfig, minecraft::login::{Result, yggdrasil::{Profile, YggdrasilAccount, YggdrasilAccountTrait, YggdrasilAuthInfo}}, utils::download::{check_hash, download}};

use super::YggdrasilUserData;

/// An account with standard Yggdrasil, using [Authlib-Injector](https://github.com/yushijinhun/authlib-injector)
#[derive(Serialize, Deserialize, Clone)]
pub struct AuthlibInjectorAccount;

#[derive(Deserialize)]
struct BMCLAPIAuthlibInjectorResponse {
    download_url: String,
    checksums: BMCLAPIAuthlibInjectorChecksums
}

#[derive(Deserialize)]
struct BMCLAPIAuthlibInjectorChecksums {
    sha256: String
}

impl YggdrasilAccountTrait for AuthlibInjectorAccount {
    async fn prepare_launch(&self, version_launch_dir: &Path, launcher: &LauncherConfig) -> Result<()> {
        let path = version_launch_dir.join("authlib-injector-latest.jar");
        let release_info: BMCLAPIAuthlibInjectorResponse = launcher.http_client
            .get("https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json")
            .send().await?.json().await?;
        if check_hash::<Sha256>(&path, Some(&release_info.checksums.sha256), 0) {
            return Ok(());
        }
        download(release_info.download_url, path).await?;
        Ok(())
    }

    async fn get_launch_jvmargs(&self, data: &YggdrasilUserData, launcher: &LauncherConfig) -> Result<Vec<OsString>> {
        let content = launcher.http_client.get(data.api_url.clone()).send().await?.bytes().await?;
        Ok(vec![
            OsString::from(format!("-javaagent:./authlib-injector-latest.jar={}", data.api_url)),
            OsString::from(format!("-Dauthlibinjector.yggdrasil.prefetched={}", BASE64_STANDARD.encode(content)))
        ])
    }
}

impl AuthlibInjectorAccount {
    /// Create a [AuthlibInjectorAccount] with authenicated tokens and selected profile from [yggdrasil::auth]
    pub fn new(auth_info: YggdrasilAuthInfo, profile: Profile) -> YggdrasilAccount<AuthlibInjectorAccount> {
        YggdrasilAccount {
            data: YggdrasilUserData::new(auth_info, profile),
            extra: Self
        }
    }
}
