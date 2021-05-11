use std::path::PathBuf;

use anyhow::Result;
use env_logger::Env;
use rabbit_digger::{controller, RabbitDigger};
use structopt::StructOpt;

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

    let controller = controller::Controller::new();

    let rabbit_digger = RabbitDigger::new(args.config)?;
    rabbit_digger.run(&controller).await?;

    Ok(())
}

#[paw::main]
#[async_std::main]
async fn main(args: Args) -> Result<()> {
    match real_main(args).await {
        Ok(()) => {}
        Err(e) => log::error!("Process exit: {:?}", e),
    }
    Ok(())
}
