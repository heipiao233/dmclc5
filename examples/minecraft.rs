use std::{path::Path, process::Stdio};

use anyhow::Result;
use dmclc5::{minecraft::schemas::VersionList, utils::DownloadAllMessage, LauncherContext, StdioUserInterface};
use tokio::{process::Command, sync::mpsc};

async fn handle_msg(msg: DownloadAllMessage, count: &mut usize) {
    match msg {
        DownloadAllMessage::Started(urls) => {
            *count = urls.len();
            println!("Download starts with: {} ({count})", urls.join("\n"));
        }
        DownloadAllMessage::SingleFinished(url) => {
            *count -= 1;
            println!("{} finished. ({count} remaining)", url);
        }

        DownloadAllMessage::SingleError(res, error) => {
            *count -= 1;
            println!("{} finished with error {}. ({count} remaining)", res.url, error);
        }
    }
}

async fn real_main() -> Result<()> {
    let vers = VersionList::get_list().await?;
    let launcher = LauncherContext::new(Path::new("./test"), StdioUserInterface).await?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let message_handler = async move {
        let mut count = 0;
        while let Some(next) = rx.recv().await {
            handle_msg(next, &mut count).await;
        }
    };
    let mc = vers.find_by_id("1.20.6").unwrap().install(&launcher, "1.20.6", tx);
    let mc = tokio::join!(message_handler, mc).1?;
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
    let (tx, mut rx) = mpsc::unbounded_channel();
    let msg_handler = async move {
        let mut count = 0;
        while let Some(next) = rx.recv().await {
            handle_msg(next, &mut count).await;
        }
    };
    let args = mc.launch_args(account, tx);
    let args = tokio::join!(msg_handler, args).1?;
    Command::new(mc.extra_data.with_java.as_ref().map_or("java", String::as_str))
        .args(args)
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
