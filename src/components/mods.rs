//! Things about mod loaders and dependencies.

pub mod fabric;
pub mod quilt;
pub mod new_forgelike;
pub mod old_forge;

use std::{collections::HashMap, ffi::OsString, fmt::{Debug, Display, Write, format}, path::{Path, PathBuf}};

use enum_dispatch::enum_dispatch;
use join_string::Join;
use std::fs;
use versions::Versioning;

use crate::{components::{install::ComponentInstallerTrait, mods::{fabric::FabricModLoader, new_forgelike::NewerForgeLikeModLoader, old_forge::OldForgeModLoader, quilt::QuiltModLoader}}, minecraft::version::MinecraftInstallation};

/// A version requirement.
/// If all the [versions::Requirement] matches, the [VersionBound] will match.
#[derive(Clone, Debug)]
pub enum VersionBound {
    None,
    One(versions::Requirement),
    Two(versions::Requirement, versions::Requirement)
}

impl VersionBound {
    /// Check if a [versions::Versioning] matches all the [versions::Requirement]s.
    pub fn matches(&self, version: &versions::Versioning) -> bool {
        return match self {
            Self::None => true,
            Self::One(req) => req.matches(version),
            Self::Two(a, b) => a.matches(version) && b.matches(version),
        };
    }

    /// Create a [VersionBound] with only one [versions::Requirement].
    pub fn new_one(req: versions::Requirement) -> Self {
        Self::One(req)
    }
}

impl ToString for VersionBound {
    fn to_string(&self) -> String {
        match self {
            Self::None => String::from("Any"),
            Self::One(v) => v.to_string(),
            Self::Two(a, b) => format!("{a} {b}")
        }
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
            f.write_str(&self.version.iter().map(ToString::to_string).join(&format!(" {} ", t!("or"))).into_string())?;
        }
        if !self.unless.is_empty() {
            write!(f, ", {} ", t!("dependencies.unless", unless = self.unless.iter().map(ToString::to_string).join(format!(" {} ", t!("or")))))?;
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

/// A dependency warning or error.
#[derive(Debug)]
pub enum ModIssue {
    DependsMissing(String, DepRequirement),
    RecommendsMissing(String, DepRequirement),
    SuggestsMissing(String, DepRequirement),
    BreaksExist(String, DepRequirement),
    ConflictsExist(String, DepRequirement),
}

/// The interface for [ModLoader].
#[enum_dispatch(ModLoader)]
pub trait ModLoaderTrait {
    /// Get the builtin mods.
    fn get_builtin_mods(&self) -> Vec<ModInfo>;
    /// Get the mods in a file.
    fn get_mods_in_file(&self, path: &Path) -> Result<Vec<ModInfo>>;
}

/// Represents a mod loader like FML, Fabric Loader and Quilt Loader.
/// In DMCLC5 it can scan a mod file and give out mod metadata.
#[enum_dispatch]
pub enum ModLoader {
    /// Fabric's mostly-version-independent mod loader.
    /// https://github.com/FabricMC/fabric-loader
    FabricModLoader,
    /// The mod-loader that cares.
    /// A fork of Fabric Loader.
    /// https://github.com/QuiltMC/quilt-loader
    QuiltModLoader,
    /// MinecraftForge's mod Loader, FML, before 1.14.
    /// https://github.com/MinecraftForge/FML
    /// https://github.com/MinecraftForge/MinecraftForge
    OldForgeModLoader,
    /// MinecraftForge after 1.14 or NeoForge's mod Loader, FML.
    /// https://github.com/MinecraftForge/MinecraftForge
    /// https://github.com/neoforged/FancyModLoader
    NewerForgeLikeModLoader
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
        let name = i.name.as_ref().unwrap_or(&i.id);
        for depend in &i.depends {
            if !check_mod_dependency(mods, depend, false) {
                issues.push(ModIssue::DependsMissing(name.clone(), depend.clone()));
            }
        }
        for depend in &i.recommends {
            if !check_mod_dependency(mods, depend, false) {
                issues.push(ModIssue::RecommendsMissing(name.clone(), depend.clone()));
            }
        }
        for depend in &i.suggests {
            if !check_mod_dependency(mods, depend, false) {
                issues.push(ModIssue::SuggestsMissing(name.clone(), depend.clone()));
            }
        }
        for depend in &i.conflicts {
            if !check_mod_dependency(mods, depend, true) {
                issues.push(ModIssue::ConflictsExist(name.clone(), depend.clone()));
            }
        }
        for depend in &i.breaks {
            if !check_mod_dependency(mods, depend, true) {
                issues.push(ModIssue::BreaksExist(name.clone(), depend.clone()));
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
            let loader = i.name.get_mod_loaders(&i.version, &self.config).await?;
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
    pub async fn list_mods(&self) -> Result<HashMap<OsString, HashMap<String, ModInfo>>> {
        if let None = self.extra_data.version {
            return Err(ModsError::MinecraftVersionUnknown);
        }
        let mut mods: HashMap<OsString, HashMap<String, ModInfo>> = HashMap::new();
        let moddir: PathBuf = Path::join(&self.version_launch_work_dir, "mods");
        let mut loaders = vec![];
        for i in &self.extra_data.components {
            let loader = i.name.get_mod_loaders(&i.version, &self.config).await?;
            loaders.extend(loader);
        }
        for file in fs::read_dir(&moddir)? {
            if let Ok(file) = file && !file.file_type()?.is_dir() {
                let mut mods_in_file = HashMap::new();
                for l in &loaders {
                    let infos = l.get_mods_in_file(&Path::join(&moddir, file.file_name()))?;
                    for info in infos {
                        mods_in_file.insert(info.id.clone(), info);
                    }
                }
                mods.insert(file.file_name(), mods_in_file);
            }
        }
        Ok(mods)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ModsError {
    #[error("Download Error: {0}")]
    DownloadError(#[from] crate::utils::download::DownloadError),
    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Zip File Error: {0}")]
    ZipError(#[from] zip::result::ZipError),
    #[error("JSON Serialize/Deserialize Error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("TOML Read Error: {0}")]
    TomlError(#[from] toml::de::Error),
    #[error("Maven version bound parse error: {0}")]
    MavenVersionBoundError(String),
    #[error("Mod {0} specified version as jar version, but there is no Implementation-Version in the jar")]
    JarVersionMissingError(String),
    #[error("Minecraft version unknown")]
    MinecraftVersionUnknown,
}

pub type Result<T> = std::result::Result<T, ModsError>;
