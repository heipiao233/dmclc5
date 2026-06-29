use std::path::Path;

use dmclc5::{LauncherContext, content_services::{Content, ContentService, ContentVersion, modrinth::ModrinthContentService}};

#[tokio::main]
async fn main() {
    let launcher: LauncherContext = LauncherContext::new(Path::new("./test"), "dmclc example contents".to_string()).await.unwrap();
    println!("{:?}", ModrinthContentService.search_content("create", 0, 10, dmclc5::content_services::ContentType::Mod, 2, None, &launcher).await.unwrap()[0]
        .list_downloadable_versions(None, &launcher).await.unwrap()[0]
        .get_version_file_name());
}
