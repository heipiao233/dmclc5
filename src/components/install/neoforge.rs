#[cfg(feature = "mod_loaders")]
use crate::components::mods::ModLoader;
use crate::{LauncherContext, components::{install::{ComponentInstaller, forgelike::ForgeLikeInstaller}, mods::{ModInfo, new_forgelike::NewerForgeLikeModLoader}}, minecraft::schemas::{Argument, VersionJSON}};

use super::forgelike::ForgeLikeInstallerTrait;

#[derive(Clone, Copy)]
pub(crate) struct NeoForgeInstaller;
pub const NEOFORGE_INSTALLER: ComponentInstaller = ComponentInstaller::NeoForge(ForgeLikeInstaller(NeoForgeInstaller));

impl ForgeLikeInstallerTrait for NeoForgeInstaller {
    const SUPPORTS_OLDER_VERSION: bool = false;
    const MAVEN_GROUP_URL: &'static str = "https://maven.neoforged.net/releases/net/neoforged";

    #[cfg(feature = "mod_loaders")]
    fn get_mod_loaders(version: &str, _: &LauncherContext) -> Vec<ModLoader> {
        let id = if version.starts_with("1.20.1-") {
            "forge".to_string()
        } else {
            "neoforge".to_string()
        };
        let mods_toml_name = if version.starts_with("1.20.1-") || (version.starts_with("20.") && ['2', '3', '4'].contains(&version.chars().collect::<Vec<char>>()[3])) {
            "mods.toml".to_string()
        } else {
            "neoforge.mods.toml".to_string()
        };
        let loader = NewerForgeLikeModLoader {
            builtin_mod: ModInfo {
                name: Some("NeoForge".to_string()),
                id,
                version: Some(versions::Versioning::new(version).unwrap()),
                desc: Some("NeoForge, a NEW broad compatibility API.".to_string()),
                license: "LGPL-2.1".to_string(),
                depends: vec![],
                recommends: vec![],
                suggests: vec![],
                conflicts: vec![],
                breaks: vec![],
            },
            mods_toml_name
        };
        vec![loader.into()]
    }

    fn find_in_version(mc: &VersionJSON) -> Option<String> {
        if let VersionJSON::New { arguments, base: _ } = mc {
            for arg2 in arguments.game.as_ref()?.windows(2) {
                if let Argument::String(v) = &arg2[0] && v == "--fml.neoForgeVersion" && let Argument::String(w) = &arg2[1] {
                    return Some(w.to_string());
                }
            }
        }
        None
    }

    fn get_archive_base_name(mc_version: &str) -> String {
        if mc_version == "1.20.1" {
            "forge".to_string()
        } else {
            "neoforge".to_string()
        }
    }

    fn match_version(loader: &str, mc: &str) -> bool {
        if mc == "1.20.1" {
            loader.starts_with("1.20.1-")
        } else if mc.contains("-") || mc.contains("w") {
            false
        } else {
            loader.starts_with(&mc.chars().skip(2).collect::<String>())
        }
    }
}
