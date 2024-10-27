use crate::{components::mods::{new_forgelike::NewerForgeLikeModLoader, old_forge::OldForgeModLoader, ModInfo, ModLoader}, minecraft::schemas::{Argument, VersionJSON}, LauncherContext};

use super::forgelike::ForgeLikeInstaller;

pub(crate) struct ForgeInstaller;

impl ForgeLikeInstaller for ForgeInstaller {

    #[cfg(feature = "mod_loaders")]
    fn get_mod_loaders(&self, version: &str, _: &LauncherContext) -> Vec<Box<dyn ModLoader>> {
        if version.split(".").collect::<Vec<&str>>()[1].parse::<usize>().unwrap() <= 13 {
            vec![Box::new(OldForgeModLoader {
                version: version.split("-").collect::<Vec<_>>()[1].to_string()
            })]
        } else {
            let loader = NewerForgeLikeModLoader {
                builtin_mod: ModInfo {
                    name: Some("Forge".to_string()),
                    id: "forge".to_string(),
                    version: Some(versions::Versioning::new(version.split("-").collect::<Vec<&str>>()[1]).unwrap()),
                    desc: Some("Forge, a broad compatibility API.".to_string()),
                    license: "LGPL-2.1".to_string(),
                    depends: vec![],
                    recommends: vec![],
                    suggests: vec![],
                    conflicts: vec![],
                    breaks: vec![],
                },
                mods_toml_name: "mods.toml".to_string()
            };
            vec![Box::new(loader)]
        }
    }

    fn supports_older_version(&self) -> bool {
        true
    }

    fn find_in_version(&self, mc: &VersionJSON) -> Option<String> {
        for l in &mc.get_base().libraries {
            let coord = &l.get_base().name;
            if ["fmlloader", "forge"].contains(&coord.name.as_str()) {
                return Some(coord.version.clone().split("-").collect::<Vec<&str>>()[1].to_string());
            }
        }

        if let VersionJSON::New { arguments, base: _ } = mc {
            for arg2 in arguments.game.as_ref()?.windows(2) {
                if let Argument::String(v) = &arg2[0] && v == "--fml.forgeVersion" && let Argument::String(w) = &arg2[1] {
                    return Some(w.to_string());
                }
            }
        }
        None
    }

    fn get_maven_group_url(&self) -> String {
        return "https://maven.minecraftforge.net/net/minecraftforge".to_string();
    }

    fn get_archive_base_name(&self, _mc_version: &str) -> String {
        "forge".to_string()
    }

    fn match_version(&self, loader: &str, mc: &str) -> bool {
        loader.starts_with(&(mc.to_owned() + "-"))
    }
}
