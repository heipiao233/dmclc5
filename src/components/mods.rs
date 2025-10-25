//! Things about mod loaders and dependencies.

pub mod fabric;
pub mod quilt;
pub mod new_forgelike;
pub mod old_forge;

use std::{collections::HashMap, fmt::{Debug, Display, Write}};

use anyhow::{anyhow, Result};
use tokio::fs;
use versions::Versioning;

use crate::{minecraft::version::MinecraftInstallation, utils::BetterPath};

/// A version requirement.
/// If all the [versions::Requirement] matches, the [VersionBound] will match.
#[derive(Clone, Debug)]
pub struct VersionBound(pub Vec<versions::Requirement>);

impl VersionBound {
    /// Check if a [versions::Versioning] matches all the [versions::Requirement]s.
    pub fn matches(&self, version: &versions::Versioning) -> bool {
        return self.0.iter().all(|v|v.matches(version));
    }

    /// Create a [VersionBound] with only one [versions::Requirement].
    pub fn new_one(req: versions::Requirement) -> Self {
        Self(vec![req])
    }
}

impl ToString for VersionBound {
    fn to_string(&self) -> String {
        self.0.iter().map(ToString::to_string).intersperse(" ".to_string()).collect()
    }
}

/// A dependency requirement.
#[derive(Clone)]
pub struct DepRequirement {
    /// Required modid.
    pub id: String,
    /// Required mod version range.
    pub version: Vec<VersionBound>,
    /// Why this is required.
    pub reason: Option<String>,
    /// If one of these met, this [DepRequirement] will be ignored.
    pub unless: Vec<DepRequirement>
}

impl Display for DepRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id)?;
        if !self.version.is_empty() {
            f.write_char(' ')?;
            f.write_str(&self.version.iter().map(ToString::to_string).intersperse(format!(" {} ", t!("or"))).collect::<String>())?;
        }
        if !self.unless.is_empty() {
            write!(f, ", {} ", t!("dependencies.unless", unless = self.unless.iter().map(ToString::to_string).intersperse(format!(" {} ", t!("or"))).collect::<String>()))?;
        }

        if let Some(v) = &self.reason {
            write!(f, ", {} ", t!("dependencies.reason", reason = v))?;
        }
        Ok(())
    }
}

impl Debug for DepRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

/// Represents a mod.
#[derive(Clone, Debug)]
pub struct ModInfo {
    /// Name.
    pub name: Option<String>,
    /// Mod ID.
    pub id: String,
    /// Mod Version.
    /// May be [Option::None] if it's provided by another mod.
    pub version: Option<versions::Versioning>,
    /// Description.
    pub desc: Option<String>,
    /// License.
    pub license: String,
    /// The dependencies. If one of them is missing, there will be a [hard](ModIssueLevel::Hard) error.
    pub depends: Vec<DepRequirement>,
    /// The recommendations. If one of them is missing, there will be a [soft](ModIssueLevel::Soft) warning.
    pub recommends: Vec<DepRequirement>,
    /// The advices. If one of them is missing, there will be a [suggestive](ModIssueLevel::Suggestive) warning.
    pub suggests: Vec<DepRequirement>,
    /// The conflicting mods. If one of them is exists, there will be a [soft](ModIssueLevel::Soft) warning.
    pub conflicts: Vec<DepRequirement>,
    /// The breaking mods. If one of them is exists, there will be a [hard](ModIssueLevel::Hard) error.
    pub breaks: Vec<DepRequirement>,
}

/// The level of a warning or error.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ModIssueLevel {
    /// Error. The game will crash.
    Hard,
    /// Warning. It still works, but may not work fine.
    Soft,
    /// Suggestive. It works fine, but it would be better if the user solves it.
    Suggestive
}

/// A dependency warning or error.
pub struct ModIssue {
    /// The level of this warning or error.
    pub level: ModIssueLevel,
    /// The message of this warning or error.
    pub message: String
}

impl Display for ModIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char('(')?;
        self.level.fmt(f)?;
        f.write_char(')')?;
        f.write_str(&self.message)?;
        Ok(())
    }
}

impl Debug for ModIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char('(')?;
        self.level.fmt(f)?;
        f.write_char(')')?;
        f.write_str(&self.message)?;
        Ok(())
    }
}

impl ModIssue {
    fn new(level: ModIssueLevel, message: &str) -> Self {
        Self {
            level,
            message: message.to_string()
        }
    }
}

/// Represents a mod loader like FML, Fabric Loader and Quilt Loader.
pub trait ModLoader {
    /// Get the builtin mods.
    fn get_builtin_mods(&self) -> Vec<ModInfo>;
    /// Get the mods in a file.
    fn get_mods_in_file(&self, path: &BetterPath) -> Result<Vec<ModInfo>>;
}

fn check_mod_dependency(mods: &HashMap<String, ModInfo>, dependency: &DepRequirement, opposite: bool) -> bool {
    for i in &dependency.unless {
        if check_mod_dependency(mods, &i, false) {
            return true;
        }
    }
    match mods.get(&dependency.id) {
        Some(mod_info) => {
            if dependency.version.is_empty() {
                return !opposite;
            }
            for i in &dependency.version {
                if mod_info.version.is_none() || i.matches(&mod_info.version.as_ref().unwrap()) {
                    return !opposite;
                }
            }
            return opposite;
        }
        None => {
            return opposite;
        }
    }
}

/// Check if all the mod dependencies are met.
/// 
/// # Arguments
/// * `mods` - A HashMap, the key is mod id, and the value is [ModInfo].
pub fn check_mod_dependencies(mods: &HashMap<String, ModInfo>) -> Vec<ModIssue> {
    let mut issues = Vec::new();
    for i in mods.values() {
        for depend in &i.depends {
            if !check_mod_dependency(mods, depend, false) {
                issues.push(ModIssue::new(ModIssueLevel::Hard, &t!("dependencies.dependency_unmet", source = i.name.as_ref().unwrap_or(&i.id), relation = t!("dependencies.relation.depends"), dependency = depend)))
            }
        }
        for depend in &i.recommends {
            if !check_mod_dependency(mods, depend, false) {
                issues.push(ModIssue::new(ModIssueLevel::Soft, &t!("dependencies.dependency_unmet", source = i.name.as_ref().unwrap_or(&i.id), relation = t!("dependencies.relation.recommends"), dependency = depend)))
            }
        }
        for depend in &i.suggests {
            if !check_mod_dependency(mods, depend, false) {
                issues.push(ModIssue::new(ModIssueLevel::Suggestive, &t!("dependencies.dependency_unmet", source = i.name.as_ref().unwrap_or(&i.id), relation = t!("dependencies.relation.suggests"), dependency = depend)))
            }
        }
        for depend in &i.conflicts {
            if !check_mod_dependency(mods, depend, true) {
                issues.push(ModIssue::new(ModIssueLevel::Soft, &t!("dependencies.dependency_unmet", source = i.name.as_ref().unwrap_or(&i.id), relation = t!("dependencies.relation.conflicts"), dependency = depend)))
            }
        }
        for depend in &i.breaks {
            if !check_mod_dependency(mods, depend, true) {
                issues.push(ModIssue::new(ModIssueLevel::Hard, &t!("dependencies.dependency_unmet", source = i.name.as_ref().unwrap_or(&i.id), relation = t!("dependencies.relation.breaks"), dependency = depend)))
            }
        }
    }
    issues
}

impl MinecraftInstallation<'_> {
    /// Check if all the mod dependencies are met in the [MinecraftInstallation].
    pub async fn check_mod_dependencies(&self) -> Result<Vec<ModIssue>> {
        let mut mods: HashMap<String, ModInfo> = self.list_mods().await?.into_values().flat_map(HashMap::into_iter).collect();
        let mut loaders = vec![];
        for i in &self.extra_data.components {
            let v = &*self.launcher.component_installers[&i.name];
            let loader = v.get_mod_loaders(&i.version, self.launcher).await?;
            for l in &loader {
                for m in l.get_builtin_mods() {
                    mods.insert(m.id.clone(), m);
                }
            }
            loaders.extend(loader);
        }
        mods.insert("minecraft".to_string(), ModInfo {
            name: Some("Minecraft".to_string()),
            id: "minecraft".to_string(),
            version: Some(Versioning::new(self.extra_data.version.as_ref().unwrap()).unwrap()),
            desc: Some("Minecraft, the game".to_string()),
            license: "ARR".to_string(),
            depends: vec![],
            recommends: vec![],
            suggests: vec![],
            conflicts: vec![],
            breaks: vec![],
        });
        Ok(check_mod_dependencies(&mods))
    }

    /// Get the mod list.
    /// 
    /// # Returns
    /// A HashMap. The key is file name, and the value is a HashMap that contains all the mod IDs and [ModInfo]s in this file.
    pub async fn list_mods(&self) -> Result<HashMap<String, HashMap<String, ModInfo>>> {
        if let None = self.extra_data.version {
            return Err(anyhow!(t!("loaders.minecraft_version_unknown")));
        }
        let mut loaders = vec![];
        for i in &self.extra_data.components {
            let v = &*self.launcher.component_installers[&i.name];
            let loader = v.get_mod_loaders(&i.version, self.launcher).await?;
            loaders.extend(loader);
        }
        let mut mods: HashMap<String, HashMap<String, ModInfo>> = HashMap::new();
        let moddir = &self.version_launch_work_dir / "mods";
        let mut dir = fs::read_dir(&moddir).await?;
        while let Some(file) = dir.next_entry().await? {
            if !file.file_type().await?.is_dir() {
                let mut mods_in_file = HashMap::new();
                for l in &loaders {
                    let infos = l.get_mods_in_file(&(&*moddir / file.file_name()))?;
                    for info in infos {
                        mods_in_file.insert(info.id.clone(), info);
                    }
                }
                mods.insert(file.file_name().into_string().map_err(|v|anyhow!("Can't decode {v:?}"))?, mods_in_file);
            }
        }
        Ok(mods)
    }
}
