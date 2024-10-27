use std::path::Path;

use dmclc5::{LauncherContext, StdioUserInterface};

#[tokio::main]
async fn main() {
    let launcher: LauncherContext = LauncherContext::new(Path::new("./test"), StdioUserInterface).await.unwrap();
    println!("{:?}", &launcher.content_services["modrinth"].search_content("create".to_string(), 0, 10, dmclc5::content_services::ContentType::Mod, 2, None, &launcher).await.unwrap()[0].list_downloadable_versions(None, &launcher).await.unwrap()[0].get_version_file_name());
}
