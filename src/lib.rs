#![warn(missing_docs)]
#![feature(let_chains)]
#![feature(iter_intersperse)]
#![feature(iter_array_chunks)]

//! A Minecraft launcher library.

use std::{collections::HashMap, io::Write, path::Path};

use anyhow::{Ok, Result};
use async_trait::async_trait;
#[cfg(feature="mod_loaders")]
use components::install::{fabriclike::FabricLikeInstaller, forge::ForgeInstaller, neoforge::NeoForgeInstaller, ComponentInstaller};
#[cfg(feature="content_services")]
use content_services::{ContentService, curseforge::CurseforgeContentService, modrinth::ModrinthContentService};
use futures_util::StreamExt;
use map_macro::hash_map_e;
#[cfg(feature="msa_auth")]
use minecraft::login::microsoft::MicrosoftAccountConstructor;
use minecraft::{login::{yggdrasil::{ali::AuthlibInjectorAccountConstructor, mul::MinecraftUniversalLoginAccountConstructor}, AccountConstructor, OfflineAccountConstructor}, schemas::VersionJSON, version::MinecraftInstallation};
use reqwest::Client;
use tokio::{fs::{self, create_dir_all}, io::AsyncWriteExt};
use tokio_util::codec::{FramedRead, LinesCodec};
use utils::{osstr_concat, BetterPath};
#[macro_use]
extern crate rust_i18n;

#[cfg(feature="content_services")]
pub mod content_services;
pub mod minecraft;
pub mod utils;
pub mod components;

i18n!("locales");

const CACHEDIR_TAG: &str = r"Signature: 8a477f597d28d172789f06886806bc55
# This file is a cache directory tag created by a Minecraft launcher.
# For information about cache directory tags, see:
#	http://www.brynosaurus.com/cachedir/";
/// The core struct for DMCLC.
/// It contains everything we need.
pub struct LauncherContext {
    root_path: BetterPath,
    #[cfg(feature="msa_auth")]
    ms_client_id: String,
    http_client: Client,
    ui: Box<dyn UserInterface>,
    #[cfg(feature="components_installation")]
    pub(crate) component_installers: HashMap<String, Box<dyn ComponentInstaller>>,
    #[cfg(feature="content_services")]
    /// A HashMap of [ContentService]s.
    pub content_services: HashMap<String, Box<dyn ContentService>>,
    /// A HashMap of [AccountConstructor]s.
    pub account_types: HashMap<String, Box<dyn AccountConstructor>>
}

/// A trait for interacting with users that should be implemented by the client.
/// # Examples
/// See [StdioUserInterface] for example.
#[async_trait]
pub trait UserInterface: Send + Sync {
    /// Asks the user some questions.
    /// 
    /// # Arguments
    /// * `questions` - A vector of questions. The first element in the tuple is the keys of the return value. and the second is what you should show to your user.
    /// * `msg` - An optional message to user.
    /// 
    /// # Returns
    /// A HashMap. The keys are the first element of each item in the argument `questions`, the values is the answers from the user for each questions.
    async fn ask_user(&self, questions: Vec<(&str, &str)>, msg: Option<&str>) -> HashMap<String, String>;

    /// Asks the user a question.
    async fn ask_user_one(&self, question: &str, msg: Option<&str>) -> String;

    /// Asks the user to choose one choice.
    /// 
    /// # Arguments
    /// * `msg` - The question.
    /// 
    /// # Returns
    /// The index of `choices`.
    async fn ask_user_choose(&self, choices: Vec<&str>, msg: &str) -> usize;

    /// Shows a information to the user.
    async fn info(&self, msg: &str, title: &str);

    /// Shows a warning to the user.
    async fn warn(&self, msg: &str, title: &str);

    /// Shows an error to the user.
    async fn error(&self, msg: &str, title: &str);
}

/// An example implementation of [UserInterface]
/// It is for some simple use cases.
pub struct StdioUserInterface;

#[async_trait]
impl UserInterface for StdioUserInterface {
    async fn ask_user(&self, questions: Vec<(&str, &str)>, msg: Option<&str>) -> HashMap<String, String> {
        if msg.is_some() {
            println!("{}", msg.unwrap());
        }
        let mut res = HashMap::<String, String>::new();
        let mut stdin = FramedRead::new(tokio::io::stdin(), LinesCodec::new());
        for (k, v) in questions {
            println!("{v}: ");
            res.insert(k.to_string(), stdin.next().await.unwrap().unwrap());
        }
        res
    }
    async fn ask_user_one(&self, question: &str, msg: Option<&str>) -> String {
        if msg.is_some() {
            println!("{}", msg.unwrap());
        }
        println!("{question}: ");
        let mut stdin = FramedRead::new(tokio::io::stdin(), LinesCodec::new());
        return stdin.next().await
            .unwrap().unwrap().to_string();
    }

    async fn ask_user_choose(&self, choices: Vec<&str>, msg: &str) -> usize {
        println!("{msg}");
        let mut index = 0;
        for i in choices {
            println!("{index}. {i}");
            index += 1;
        }
        println!("Please choose: ");
        let mut stdin = FramedRead::new(tokio::io::stdin(), LinesCodec::new());
        return stdin.next().await.unwrap().unwrap().parse().unwrap();
    }

    async fn info(&self, msg: &str, title: &str) {
        println!("INFO: {title} {msg}");
    }
    async fn warn(&self, msg: &str, title: &str) {
        eprintln!("WARN: {title} {msg}");
    }
    async fn error(&self, msg: &str, title: &str) {
        eprintln!("ERROR: {title} {msg}");
    }
}

impl LauncherContext {

    /// Creates a new [LauncherContext].
    /// 
    /// # Arguments
    /// * `root_path` - The `.minecraft` directory.
    #[cfg_attr(feature="msa_auth", doc=r" * `ms_client_id` - The client id for Microsoft auth. See [Microsoft's document](https://docs.microsoft.com/en-us/azure/active-directory/develop/quickstart-register-app).")]
    /// * `ui` - Your implementation of [UserInterface].
    pub async fn new(root_path: &Path, #[cfg(feature="msa_auth")] ms_client_id: &str, ui: impl UserInterface + 'static) -> Result<Self> {
        let root_path = BetterPath(root_path.to_path_buf().canonicalize()?);
        tokio::fs::File::create(&root_path / "libraries" / "CACHEDIR.TAG").await?.write(CACHEDIR_TAG.as_bytes()).await?;
        tokio::fs::File::create(&root_path / "assets" / "CACHEDIR.TAG").await?.write(CACHEDIR_TAG.as_bytes()).await?;
        #[allow(unused_mut)]
        let mut ctx = LauncherContext {
            root_path,
            #[cfg(feature="msa_auth")]
            ms_client_id: ms_client_id.to_string(),
            http_client: Client::builder().user_agent("heipiao233/dmclc5 (heipiao233@outlook.com)").build()?,
            ui: Box::new(ui),
            #[cfg(feature="mod_loaders")]
            component_installers: hash_map_e!{
                "forge".to_string() => Box::new(ForgeInstaller),
                "neoforge".to_string() => Box::new(NeoForgeInstaller),
                "fabric".to_string() => Box::new(FabricLikeInstaller::fabric()),
                "quilt".to_string() => Box::new(FabricLikeInstaller::quilt()),
            },
            #[cfg(feature="content_services")]
            content_services: hash_map_e! {
                "curseforge".to_string() => Box::new(CurseforgeContentService),
                "modrinth".to_string() => Box::new(ModrinthContentService)
            },
            #[cfg(feature="msa_auth")]
            account_types: hash_map_e! {
                "offline".to_string() => Box::new(OfflineAccountConstructor),
                "microsoft".to_string() => Box::new(MicrosoftAccountConstructor),
                "minecraft_universal_login".to_string() => Box::new(MinecraftUniversalLoginAccountConstructor),
                "authlib_injector".to_string() => Box::new(AuthlibInjectorAccountConstructor)
            },
            #[cfg(not(feature="msa_auth"))]
            account_types: hash_map_e! {
                "offline".to_string() => Box::new(OfflineAccountConstructor),
                "minecraft_universal_login".to_string() => Box::new(MinecraftUniversalLoginAccountConstructor),
                "authlib_injector".to_string() => Box::new(AuthlibInjectorAccountConstructor)
            }
        };
        Ok(ctx)
    }
    
    /// List the names of minecraft installations in the `root_path`.
    pub async fn list_installations(&self) -> Result<Vec<String>> {
        let version_dir = &*(&self.root_path / "versions");
        let mut ret = Vec::new();
        create_dir_all(version_dir).await?;
        for i in std::fs::read_dir((&self.root_path / "versions").0)? {
            let dir = i?;
            if !dir.file_type()?.is_dir() {
                continue;
            }
            let json_path = version_dir / dir.file_name() / osstr_concat(&dir.file_name(), &".json".to_string());
            let m = std::fs::metadata(&json_path);
            if let Err(_) = m {
                continue;
            }
            if !m.unwrap().is_file() {
                continue;
            }
            ret.push(dir.file_name().to_string_lossy().to_string());
        }
        Ok(ret)
    }

    /// Get one [MinecraftInstallation] by name in the `root_path`.
    pub async fn get_installation(&self, name: &str) -> Option<MinecraftInstallation> {
        let version_dir = &*(&self.root_path / "versions" / name);
        let meta = fs::metadata(version_dir).await;
        if meta.is_err() || !meta.unwrap().is_dir() {
            return None;
        }
        let json = fs::read(version_dir / (name.to_string() + ".json")).await.ok()?;
        let json: VersionJSON = serde_json::from_slice(&json).ok()?;
        Some(MinecraftInstallation::new(self, json, name, None))
    }

    /// Set a new `root_path`.
    pub fn set_root_path(&mut self, root_path: &Path) -> Result<()> {
        self.root_path = BetterPath(root_path.to_path_buf().canonicalize()?);
        std::fs::File::create(&self.root_path / "libraries" / "CACHEDIR.TAG")?.write(CACHEDIR_TAG.as_bytes())?;
        std::fs::File::create(&self.root_path / "assets" / "CACHEDIR.TAG")?.write(CACHEDIR_TAG.as_bytes())?;
        Ok(())
    }
}
