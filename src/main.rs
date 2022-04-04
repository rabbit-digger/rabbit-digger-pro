use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use rabbit_digger::{config::Config, RabbitDigger, Registry};
use tokio::fs::read_to_string;

#[derive(Parser)]
struct Args {
    /// Path to config file
    #[clap(
        short,
        long,
        env = "RD_CONFIG",
        parse(from_os_str),
        default_value = "config.yaml"
    )]
    config: PathBuf,
}

async fn real_main(args: Args) -> Result<()> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "rabbit_digger=debug")
    }
    tracing_subscriber::fmt::init();

    let content = read_to_string(args.config).await?;
    let config: Config = serde_yaml::from_str(&content)?;

    let registry = Registry::new_with_builtin()?;
    let rd = RabbitDigger::new(registry).await?;
    rd.start(config).await?;
    rd.join().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match real_main(args).await {
        Ok(()) => {}
        Err(e) => tracing::error!("Process exit: {:?}", e),
    }
    Ok(())
}
