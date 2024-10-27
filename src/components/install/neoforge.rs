use crate::{components::mods::{new_forgelike::NewerForgeLikeModLoader, ModInfo, ModLoader}, minecraft::schemas::{Argument, VersionJSON}, LauncherContext};

use super::forgelike::ForgeLikeInstaller;

pub(crate) struct NeoForgeInstaller;

impl ForgeLikeInstaller for NeoForgeInstaller {
    #[cfg(feature = "mod_loaders")]
    fn get_mod_loaders(&self, version: &str, _: &LauncherContext) -> Vec<Box<dyn ModLoader>> {
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
        vec![Box::new(loader)]
    }

    fn supports_older_version(&self) -> bool {
        false
    }

    fn find_in_version(&self, mc: &VersionJSON) -> Option<String> {
        if let VersionJSON::New { arguments, base: _ } = mc {
            for arg2 in arguments.game.as_ref()?.windows(2) {
                if let Argument::String(v) = &arg2[0] && v == "--fml.neoForgeVersion" && let Argument::String(w) = &arg2[1] {
                    return Some(w.to_string());
                }
            }
        }
        None
    }

    fn get_maven_group_url(&self) -> String {
        return "https://maven.neoforged.net/releases/net/neoforged".to_string();
    }

    fn get_archive_base_name(&self, mc_version: &str) -> String {
        if mc_version == "1.20.1" {
            "forge".to_string()
        } else {
            "neoforge".to_string()
        }
    }

    fn match_version(&self, loader: &str, mc: &str) -> bool {
        if mc == "1.20.1" {
            loader.starts_with("1.20.1-")
        } else if mc.contains("-") || mc.contains("w") {
            false
        } else {
            loader.starts_with(&mc.chars().skip(2).collect::<String>())
        }
    }
}
