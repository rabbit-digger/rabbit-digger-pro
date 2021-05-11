use std::path::PathBuf;

use anyhow::Result;
use env_logger::Env;
use rabbit_digger::{config::Config, controller, RabbitDigger};
use structopt::StructOpt;
use tokio::fs::read_to_string;

#[derive(StructOpt)]
struct Args {
    /// Path to config file
    #[structopt(
        short,
        long,
        env = "RD_CONFIG",
        parse(from_os_str),
        default_value = "config.yaml"
    )]
    config: PathBuf,
}

async fn real_main(args: Args) -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("rabbit_digger=trace")).init();

    let content = read_to_string(args.config).await?;
    let config: Config = serde_yaml::from_str(&content)?;

    let controller = controller::Controller::new();

    let rabbit_digger = RabbitDigger::new()?;
    rabbit_digger.run(&controller, config).await?;

    Ok(())
}

#[paw::main]
#[tokio::main]
async fn main(args: Args) -> Result<()> {
    match real_main(args).await {
        Ok(()) => {}
        Err(e) => log::error!("Process exit: {:?}", e),
    }
    Ok(())
}
