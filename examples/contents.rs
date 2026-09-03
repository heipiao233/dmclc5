use std::path::PathBuf;

use dmclc5::{LauncherConfig, content_services::{Content, ContentService, ContentVersion, modrinth::ModrinthContentService}};

#[tokio::main]
async fn main() {
    let launcher: LauncherConfig = LauncherConfig::new("dmclc example contents".to_string(), PathBuf::from("./test/assets"), PathBuf::from("./test/libraries")).unwrap();
    println!("{:?}", ModrinthContentService.search_content("create", 0, 10, dmclc5::content_services::ContentType::Mod, 2, None, &launcher).await.unwrap()[0]
        .list_downloadable_versions(None, &launcher).await.unwrap()[0]
        .get_version_file_name());
}
