mod app;
mod cli;
mod config;
mod server;

use app::build_chat;
use cli::{parse_cli, Action};
use config::load_config_files;
use std::sync::Arc;

const DEFAULT_PROVIDER: &str = "demo";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    load_config_files();
    let cli = parse_cli();

    match &cli.command {
        Action::Help => {
            print!("{}", cli::USAGE);
            return Ok(());
        }
        Action::ToolRun { .. } => {}
        Action::ServerRun => {
            let provider = cli
                .provider
                .clone()
                .or_else(|| config::env_non_empty("Q_RS_DEFAULT_PROVIDER"))
                .unwrap_or_else(|| DEFAULT_PROVIDER.to_string());
            return server::server_run(build_chat(&provider), &cli);
        }
        Action::ServerStart => return server::server_start(&cli),
        Action::ServerStop => return server::server_stop(),
        Action::ServerStatus => std::process::exit(server::server_status(&cli)),
        Action::ServerInstall => return server::server_install(&cli),
        Action::Repl | Action::Run(_) => {}
    }

    let provider = cli
        .provider
        .clone()
        .or_else(|| config::env_non_empty("Q_RS_DEFAULT_PROVIDER"))
        .unwrap_or_else(|| DEFAULT_PROVIDER.to_string());
    let chat = Arc::new(build_chat(&provider));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    match cli.command {
        Action::Run(ref text) => runtime.block_on(app::run_once(chat.as_ref(), text, &cli)),
        Action::ToolRun { ref name, ref args } => {
            let tools = chat.tools().clone();
            runtime.block_on(async move {
                let val: serde_json::Value = serde_json::from_str(args).unwrap_or(serde_json::json!({}));
                match tools.execute(name, val).await {
                    Ok(out) => println!("{}", out.content),
                    Err(e) => eprintln!("erro: {e}"),
                }
            });
        }
        _ => runtime.block_on(app::run_repl(chat.as_ref(), &cli)),
    }
    Ok(())
}
