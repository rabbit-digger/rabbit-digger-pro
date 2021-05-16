#[cfg(feature = "api_server")]
mod api_server;
mod config;
mod translate;
mod util;

use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use env_logger::Env;
use futures::{
    future::{ready, TryFutureExt},
    pin_mut,
    stream::{self, StreamExt, TryStreamExt},
};
use notify_stream::{notify::RecursiveMode, notify_stream};
use rabbit_digger::{controller, RabbitDigger, Registry};
use structopt::StructOpt;
use tokio::fs::read_to_string;

use crate::util::DebounceStreamExt;

#[derive(StructOpt)]
struct ApiServer {
    /// HTTP endpoint bind address
    #[structopt(short, long, env = "RD_BIND")]
    bind: Option<String>,

    /// Access token
    #[structopt(short, long, env = "RD_ACCESS_TOKEN")]
    access_token: Option<String>,
}

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

    #[structopt(flatten)]
    api_server: ApiServer,

    /// Write generated config to path
    #[structopt(short, long, parse(from_os_str))]
    write_config: Option<PathBuf>,
}

fn plugin_loader(_cfg: &rabbit_digger::Config, registry: &mut Registry) -> Result<()> {
    // #[cfg(feature = "ss")]
    registry.init_with_registry("ss", ss::init)?;
    Ok(())
}

async fn write_config(path: PathBuf, cfg: &rabbit_digger::Config) -> Result<()> {
    let content = serde_yaml::to_string(cfg)?;
    tokio::fs::write(path, content.as_bytes()).await?;
    Ok(())
}

async fn real_main(args: Args) -> Result<()> {
    env_logger::Builder::from_env(
        Env::default()
            .default_filter_or("rabbit_digger=trace,rabbit_digger_pro=trace,rd_std=trace"),
    )
    .init();

    let controller = controller::Controller::new();

    if let Some(_bind) = args.api_server.bind {
        #[cfg(feature = "api_server")]
        api_server::Server {
            controller: controller.clone(),
            access_token: args.api_server.access_token,
        }
        .run(_bind)
        .await
        .context("Failed to run api server.")?;
    }

    let mut rabbit_digger = RabbitDigger::new()?;
    rabbit_digger.plugin_loader = Box::new(plugin_loader);

    let config_path = args.config.clone();
    let config_stream = notify_stream(&config_path, RecursiveMode::Recursive)?;
    let write_config_path = args.write_config;

    let config_stream = stream::once(async { Ok(()) })
        .chain(
            config_stream
                .try_filter(|e| ready(e.kind.is_modify()))
                .map_err(Into::<anyhow::Error>::into)
                .map(|_| Ok(()))
                .debounce(Duration::from_millis(100)),
        )
        .and_then(move |_| read_to_string(config_path.clone()).map_err(Into::into))
        .and_then(|s| ready(serde_yaml::from_str(&s).map_err(Into::into)))
        .and_then(config::post_process)
        .and_then(|c: rabbit_digger::Config| async {
            if let Some(path) = write_config_path.clone() {
                write_config(path, &c).await?;
            };
            Ok(c)
        });
    pin_mut!(config_stream);
    rabbit_digger.run_stream(&controller, config_stream).await?;

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
