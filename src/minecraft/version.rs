//! Things about a Minecraft installation.

use std::{ffi::OsString, fs, path::{Path, PathBuf}};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature="mod_loaders")]
use crate::components::{COMPONENTS, install::ComponentInstallerTrait};
#[cfg(feature="components_installation")]
use crate::components::install::ComponentInstaller;
use crate::{LauncherConfig, minecraft::prefix::MinecraftPrefix};

use super::schemas::VersionJSON;

/// Represents a component.
#[derive(Serialize, Deserialize)]
pub struct ComponentInfo {
    /// Name.
    #[serde(with = "serde_str")]
    #[cfg(feature="components_installation")]
    pub name: ComponentInstaller,
    /// Name.
    #[cfg(not(feature="components_installation"))]
    pub name: String,
    /// Version of the component.
    pub version: String
}

/// Some extra datas.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DMCLCExtraData {
    /// Stores Minecraft version.
    pub version: Option<String>,
    /// Stores components list.
    #[serde(rename = "loaders")]
    pub components: Vec<ComponentInfo>,
    /// Stores if independent game dir is enabled.
    #[serde(rename = "enableIndependentGameDir")]
    pub independent_game_dir: bool,
    /// Stores the command that should be executed before launching.
    /// It's client's responsibility to execute it.
    pub before_command: Option<String>,
    /// Stores the java command that should be used.
    /// It's client's responsibility to use it.
    #[serde(rename = "usingJava")]
    pub with_java: Option<String>,
    /// Stores appended game arguments.
    #[serde(rename = "moreGameArguments")]
    pub extra_game_arguments: Option<Vec<OsString>>,
    /// Stores appended java arguments.
    #[serde(rename = "moreJavaArguments")]
    pub extra_jvm_arguments: Option<Vec<OsString>>
}

/// Represents a Minecraft installation.
pub struct MinecraftInstallation<'c> {
    pub(crate) obj: VersionJSON,
    /// Some extra datas.
    pub extra_data: DMCLCExtraData,
    pub(crate) config: &'c LauncherConfig,
    /// Name of this installation.
    pub name: String,
    pub(crate) version_launch_work_dir: PathBuf,
    pub(crate) version_root: PathBuf
}

impl <'c> MinecraftInstallation<'c> {
    pub(crate) fn new(prefix: &MinecraftPrefix, config: &'c LauncherConfig, json: VersionJSON, name: &str, extras: Option<DMCLCExtraData>) -> Self {
        let version_root = prefix.versions_path().join(name);
        let extra_data = extras.unwrap_or_else(|| Self::get_extras(&version_root, &json, true));

        let version_launch_work_dir = if extra_data.independent_game_dir {
            version_root.clone()
        } else {
            prefix.path().to_path_buf()
        };
        Self {
            obj: json,
            extra_data,
            config,
            name: name.to_string(),
            version_launch_work_dir,
            version_root
        }
    }

    fn get_extras(version_root: &Path, object: &VersionJSON, independent_game_dir: bool) -> DMCLCExtraData {
        let path = version_root.join("dmclc_extras.json");
        if fs::metadata(&path).is_ok() && let Ok(f) = fs::File::open(&path) && let Ok(v) = serde_json::from_reader(f) {
            return v;
        }
        #[cfg(not(feature="mod_loaders"))]
        let components: Vec<ComponentInfo> = Vec::new();
        #[cfg(feature="mod_loaders")]
        let components: Vec<ComponentInfo> = COMPONENTS.iter()
            .filter_map(|c| Some((c, c.find_in_version(object)?)))
            .map(|(c, version)| ComponentInfo { name: c.clone(), version })
            .collect();
        let version = object.get_base().client_version.clone()
            .or_else(||Self::get_version_from_jar(&version_root.join(format!("{}.jar", object.get_base().id))));
        let ret = DMCLCExtraData {
            version,
            components,
            independent_game_dir,
            before_command: None,
            with_java: None,
            extra_game_arguments: None,
            extra_jvm_arguments: None
        };
        if let Ok(file) = fs::File::create(&path) {
            let _ = serde_json::to_writer(file, &ret);
        }
        ret
    }

    fn get_version_from_jar(jar_file: &Path) -> Option<String> {
        let mut archive = zip::ZipArchive::new(fs::File::open(jar_file).ok()?).ok()?;
        let obj: Value = serde_json::from_reader(archive.by_name("version.json").ok()?).ok()?;
        obj["id"].as_str().map(str::to_string)
    }
}
