//! Implementation of [ModLoader] for Fabric Loader.

use std::{collections::HashMap, fs::File, io::{Read, Seek}};

use acc_reader::AccReader;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use versions::{Requirement, Versioning};
use zip::ZipArchive;

use crate::utils::{json_newline_transform, BetterPath};

use super::{DepRequirement, ModInfo, ModLoader, VersionBound};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Env {
    #[serde(rename = "*")]
    All,
    Client,
    Server
}

#[derive(Serialize, Deserialize)]
struct NestedJarEntry {
    file: String
}
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum DependencyVersionRequirement {
    One(String),
    Many(Vec<String>)
}

pub(super) fn fabric_parse_req(req: &str) -> Requirement {
    Requirement::new(req).unwrap_or_else(||Requirement {
        op: versions::Op::Exact,
        version: Some(Versioning::new(req).unwrap())
    })
}

impl DependencyVersionRequirement {
    fn get(&self) -> Vec<Requirement> {
        match self {
            DependencyVersionRequirement::One(req) => vec![fabric_parse_req(req)],
            DependencyVersionRequirement::Many(reqs) => reqs.iter().map(String::as_str).map(fabric_parse_req).collect(),
        }
    }
}

impl Into<DepRequirement> for (String, DependencyVersionRequirement) {
    fn into(self) -> DepRequirement {
        DepRequirement {
            id: self.0,
            version: self.1.get().into_iter().map(VersionBound::new_one).collect(),
            reason: None,
            unless: vec![]
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Licenses {
    One(String),
    Many(Vec<String>)
}

impl Licenses {
    fn joined(&self) -> String {
        match &self {
            Self::One(val) => val.clone(),
            Self::Many(val) => val.join(", "),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ContactInformation {
    email: Option<String>,
    irc: Option<String>,
    homepage: Option<String>,
    issues: Option<String>,
    sources: Option<String>,
    #[serde(flatten)]
    other: HashMap<String, String>
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Person {
    Object {
        name: String,
        contact: Option<ContactInformation>
    },
    String(String)
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum Icons {
    Path(String),
    SizeToPath(HashMap<String, String>)
}

#[derive(Serialize, Deserialize)]
struct CustomData {
    #[serde(rename = "fabric-loom:generated", default)]
    fabric_loom_generated: bool
}

#[derive(Serialize, Deserialize)]
struct FabricModJson {
    id: String,
    version: String,
    provides: Option<Vec<String>>, // Not in official document
    environment: Option<Env>,
    jars: Option<Vec<NestedJarEntry>>,
    depends: Option<HashMap<String, DependencyVersionRequirement>>,
    recommends: Option<HashMap<String, DependencyVersionRequirement>>,
    suggests: Option<HashMap<String, DependencyVersionRequirement>>,
    conflicts: Option<HashMap<String, DependencyVersionRequirement>>,
    breaks: Option<HashMap<String, DependencyVersionRequirement>>,
    name: Option<String>,
    description: Option<String>,
    authors: Option<Vec<Person>>,
    contributors: Option<Vec<Person>>,
    contact: Option<ContactInformation>,
    license: Option<Licenses>,
    icon: Option<Icons>,
    custom: CustomData
}

/// The [ModLoader] implementation for Fabric Loader.
pub struct FabricModLoader {
    pub(in crate::components) builtin_mods: Option<Vec<ModInfo>>
}

impl FabricModLoader {
    fn get_mods_in_reader<R: Read + Seek>(&self, read: R) -> Result<Vec<ModInfo>> {
        let mut res = vec![];
        let mut archive = ZipArchive::new(read)?;
        let mut mod_json = String::new();
        archive.by_name("fabric.mod.json")?.read_to_string(&mut mod_json)?;
        let mod_json: FabricModJson = serde_json::from_str(&json_newline_transform(&mod_json))?;
        res.push(ModInfo {
            name: mod_json.name,
            id: mod_json.id,
            version: Some(Versioning::new(mod_json.version).unwrap()),
            desc: mod_json.description,
            license: mod_json.license.unwrap_or(Licenses::One("All Rights Reserved".to_string())).joined(),
            depends: mod_json.depends.into_iter().flat_map(HashMap::into_iter).map(Into::into).collect(),
            recommends: mod_json.recommends.into_iter().flat_map(HashMap::into_iter).map(Into::into).collect(),
            suggests: mod_json.suggests.into_iter().flat_map(HashMap::into_iter).map(Into::into).collect(),
            conflicts: mod_json.conflicts.into_iter().flat_map(HashMap::into_iter).map(Into::into).collect(),
            breaks: mod_json.breaks.into_iter().flat_map(HashMap::into_iter).map(Into::into).collect(),
        });
        for i in mod_json.provides.iter().flatten() {
            res.push(ModInfo {
                name: None,
                id: i.clone(),
                version: None,
                desc: None,
                license: res[0].license.clone(),
                depends: vec![],
                recommends: vec![],
                suggests: vec![],
                conflicts: vec![],
                breaks: vec![]
            });
        }
        for i in mod_json.jars.iter().flatten() {
            res.append(&mut self.get_mods_in_reader(AccReader::new(archive.by_name(&i.file)?))?);
        }
        Ok(res)
    }
}

impl ModLoader for FabricModLoader {
    fn get_builtin_mods(&self) -> Vec<ModInfo> {
        self.builtin_mods.clone().into_iter().flatten().collect()
    }

    fn get_mods_in_file(&self, path: &BetterPath) -> Result<Vec<ModInfo>> {
        self.get_mods_in_reader(File::open(path)?)
    }
}
