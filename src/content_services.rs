//! Things about contents.
//! Contents are things like mods, data packs, resource packs, worlds, modpacks and shaders...

pub(crate) mod modrinth;
pub(crate) mod curseforge;

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;

use crate::{minecraft::version::MinecraftInstallation, utils::BetterPath, LauncherContext};

/// Type of contents.
#[derive(PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum ContentType {
    ModPack,
    Shader,
    Mod,
    ResourcePack,
    DataPack,
    World,
}

/// Represents a screenshot.
#[allow(missing_docs)]
pub struct Screenshot {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>
}

/// Represents a `content`.
#[async_trait]
pub trait Content: Send + Sync {
    /// List downloadable versions.
    async fn list_downloadable_versions(&self, for_version: Option<&MinecraftInstallation<'_>>, launcher: &LauncherContext) -> Result<Vec<Box<dyn ContentVersion>>>;
    /// Get title.
    fn get_title(&self) -> String;
    /// Get description.
    fn get_description(&self) -> String;
    /// Get content body article in HTML.
    async fn get_body(&self, launcher: &LauncherContext) -> Result<String>;
    /// Get icon url.
    fn get_icon_url(&self) -> Option<String>;
    /// Get url for issue, Discord, source....
    /// The key is the name, the value is the url.
    fn get_urls(&self) -> HashMap<String, String>;
    /// Get the screenshots.
    fn get_screenshots(&self) -> Vec<Screenshot>;
    /// Get other informations like authors, downloads...
    fn get_other_information(&self) -> HashMap<String, String>;
    /// Check if this is a library mod.
    fn is_library(&self) -> bool;
    /// Returns the [ContentType].
    fn kind(&self) -> ContentType;
}

/**
 * A content version.
 */
#[async_trait]
pub trait ContentVersion: Send + Sync {
    /// Get file url.
    fn get_version_file_url(&self) -> String;
    /// Get file SHA1.
    fn get_version_file_sha1(&self) -> String;
    /// Get file name.
    fn get_version_file_name(&self) -> String;
    /// Get changelog in HTML.
    async fn get_version_changelog(&self, launcher: &LauncherContext) -> Result<String>;
    /// Get version number.
    fn get_version_number(&self) -> String;
    /// List the dependencies.
    async fn list_dependencies(&self, launcher: &LauncherContext) -> Result<Vec<ContentDependency>>;
}

/// Represents the dependencies.
pub enum ContentDependency
{
    /// A [Content].
    Content(Box<dyn Content>),
    /// A [ContentVersion].
    ContentVersion(Box<dyn ContentVersion>),
}

/// A service (website) that provides contents like CurseForge and Modrinth.
#[async_trait]
pub trait ContentService: Send + Sync {
    /// Search for contents.
    async fn search_content(
        &self,
        name: String,
        skip: usize,
        limit: usize,
        kind: ContentType,
        sort_field: usize,
        for_version: Option<&MinecraftInstallation<'_>>,
        launcher: &LauncherContext
    ) -> Result<Vec<Box<dyn Content>>>;
    /// Get unsupported [ContentType]s.
    fn get_unsupported_content_types(&self) -> Vec<ContentType>;
    /// Get all sort fields.
    fn get_sort_fields(&self) -> Vec<String>;
    /// Get the default sort field.
    fn get_default_sort_field(&self) -> String;
    /// Get a [ContentVersion] from a file.
    async fn get_content_version_from_file(&self, path: &BetterPath, launcher: &LauncherContext) -> Result<Option<Box<dyn ContentVersion>>>;

    /// Get a [Content] by ID.
    async fn get_content_by_id(&self, id: &str, launcher: &LauncherContext) -> Result<Option<Box<dyn Content>>>;

    /// Get a [ContentVersion] by ID.
    async fn get_content_version_by_id(&self, content_id: &str, id: &str, launcher: &LauncherContext) -> Result<Option<Box<dyn ContentVersion>>>;
}
