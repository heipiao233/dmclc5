//! Things about a Minecraft installation.

use std::{ffi::OsString, fs};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{utils::BetterPath, LauncherContext};

use super::schemas::VersionJSON;

/// Represents a component.
#[derive(Serialize, Deserialize)]
pub struct ComponentInfo {
    /// Name.
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
pub struct MinecraftInstallation<'l> {
    pub(crate) obj: VersionJSON,
    /// Some extra datas.
    pub extra_data: DMCLCExtraData,
    pub(crate) launcher: &'l LauncherContext,
    pub(crate) name: String,
    pub(crate) version_launch_work_dir: BetterPath,
    pub(crate) version_root: BetterPath
}

impl <'l> MinecraftInstallation<'l> {
    pub(crate) fn new(launcher: &'l LauncherContext, json: VersionJSON, name: &str, extras: Option<DMCLCExtraData>) -> MinecraftInstallation<'l> {
        let version_root = *(&launcher.root_path / "versions" / name);
        let extra_data = if let Some(e) = extras {
            e
        } else {
            Self::get_extras(#[cfg(feature="mod_loaders")]launcher, &version_root, &json, true)
        };
        
        let version_launch_work_dir = if extra_data.independent_game_dir {
            version_root.clone()
        } else {
            launcher.root_path.clone()
        };
        Self {
            obj: json,
            extra_data,
            launcher,
            name: name.to_string(),
            version_launch_work_dir,
            version_root
        }
    }

    fn get_extras(
        #[cfg(feature="mod_loaders")]
        launcher: &'l LauncherContext,
        version_root: &BetterPath, object: &VersionJSON, independent_game_dir: bool) -> DMCLCExtraData {
        let path = &*(version_root / "dmclc_extras.json");
        if fs::metadata(path).is_ok() && let Ok(f) = fs::File::open(path) && let Ok(v) = serde_json::from_reader(f) {
            return v;
        }
        #[allow(unused_mut)]
        let mut components: Vec<ComponentInfo> = Vec::new();
        #[cfg(feature="mod_loaders")]
        for (name, i) in &launcher.component_installers {
            if let Some(version) = i.find_in_version(object) {
                components.push(ComponentInfo { name: name.clone(), version });
            }
        }
        let version = object.get_base().client_version.clone()
            .or_else(||Self::get_version_from_jar(*(version_root / format!("{}.jar", object.get_base().id))));
        let ret = DMCLCExtraData {
            version,
            components,
            independent_game_dir,
            before_command: None,
            with_java: None,
            extra_game_arguments: None,
            extra_jvm_arguments: None
        };
        if let Ok(file) = fs::File::create(path) {
            let _ = serde_json::to_writer(file, &ret);
        }
        ret
    }
    
    fn get_version_from_jar(jar_file: BetterPath) -> Option<String> {
        let mut archive = zip::ZipArchive::new(fs::File::open(jar_file).ok()?).ok()?;
        let obj: Value = serde_json::from_reader(archive.by_name("version.json").ok()?).ok()?;
        obj["id"].as_str().map(str::to_string)
    }
}
