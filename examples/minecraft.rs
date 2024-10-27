use std::{path::Path, process::Stdio};

use anyhow::Result;
use dmclc5::{minecraft::schemas::VersionList, LauncherContext, StdioUserInterface};
use tokio::process::Command;

async fn real_main() -> Result<()> {
    let vers = VersionList::get_list().await?;
    let launcher = LauncherContext::new(Path::new("./test"), StdioUserInterface).await?;
    let mc = vers.find_by_id("1.20.6").unwrap().install(&launcher, "1.20.6").await?;
    let account = &mut *launcher.account_types["offline"].new();
    account.login(&launcher).await?;
    if let Some(c) = &mc.extra_data.before_command {
        let command: Vec<&str> = c.split(" ").collect();
        Command::new(command[0])
            .args(&command[1..])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::null())
            .current_dir(mc.get_cwd())
            .spawn()?.wait().await?;
    }
    Command::new(mc.extra_data.with_java.as_ref().map_or("java", String::as_str))
        .args(mc.launch_args(account).await?)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::null())
        .current_dir(mc.get_cwd())
        .spawn()?.wait().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    real_main().await.unwrap();
}
