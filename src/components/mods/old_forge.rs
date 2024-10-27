//! Implementation of [ModLoader] for Forge before 1.13.

use std::fs::File;

use serde::Deserialize;
use versions::{Requirement, Versioning};
use zip::ZipArchive;

use crate::{components::mods::ModInfo, utils::parse_maven_version_range};

use super::{ModLoader, VersionBound};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McmodInfoItem {
    modid: String,
    name: String,
    description: Option<String>,
    version: Option<Versioning>,
    mcversion: Option<Versioning>,
    #[serde(default)]
    use_dependency_information: bool,
    #[serde(default)]
    required_mods: Vec<String>
}

/// The [ModLoader] implementation for Forge before 1.13.
pub struct OldForgeModLoader {
    pub(in crate::components) version: String
}

impl ModLoader for OldForgeModLoader {
    fn get_builtin_mods(&self) -> Vec<super::ModInfo> {
        vec![
            ModInfo {
                name: None,
                id: "Forge".to_string(),
                version: Some(Versioning::new(&self.version).unwrap()),
                desc: None,
                license: "LGPL-2.1".to_string(),
                depends: vec![],
                recommends: vec![],
                conflicts: vec![],
                suggests: vec![],
                breaks: vec![]
            }
        ]
    }

    fn get_mods_in_file(&self, path: &crate::utils::BetterPath) -> anyhow::Result<Vec<super::ModInfo>> {
        let mut archive = ZipArchive::new(File::open(path)?)?;
        let info: Vec<McmodInfoItem> = serde_json::from_reader(archive.by_name("mcmod.info")?)?;
        let mut ret = vec![];
        for i in info {
            let mut depends = vec![];
            if i.use_dependency_information {
                for dep in i.required_mods {
                    match dep.splitn(2, "@").collect::<Vec<&str>>().as_slice() {
                        [dep, ver] => depends.push(super::DepRequirement { id: dep.to_string(), version: parse_maven_version_range(ver)?, reason: None, unless: vec![] }),
                        [dep] => depends.push(super::DepRequirement { id: dep.to_string(), version: vec![], reason: None, unless: vec![] }),
                        _ => panic!("How???")
                    }
                }
            }
            if let Some(mc) = i.mcversion {
                depends.push(super::DepRequirement {
                    id: "minecraft".to_string(),
                    version: vec![VersionBound::new_one(Requirement {op: versions::Op::Exact, version: Some(mc)})],
                    reason: None,
                    unless: vec![]
                });
            }
            ret.push(ModInfo {
                name: Some(i.name),
                id: i.modid,
                version: i.version,
                desc: i.description,
                license: "Unknown".to_string(),
                depends,
                recommends: vec![],
                suggests: vec![],
                conflicts: vec![],
                breaks: vec![]
            });
        }
        Ok(ret)
    }
}
