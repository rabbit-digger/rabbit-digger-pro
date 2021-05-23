#[cfg(feature = "api_server")]
mod api_server;
mod config;
mod schema;
mod translate;
mod util;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use env_logger::Env;
use futures::{
    future::{ready, TryFutureExt},
    pin_mut,
    stream::{self, StreamExt, TryStreamExt},
};
use notify_stream::{notify::RecursiveMode, notify_stream};
use rabbit_digger::{controller, Registry};
use structopt::StructOpt;
use tokio::fs::read_to_string;

use crate::util::DebounceStreamExt;

#[derive(StructOpt)]
struct ApiServer {
    /// HTTP endpoint bind address
    #[structopt(short, long, env = "RD_BIND")]
    bind: Option<String>,

    /// Access token
    #[structopt(long, env = "RD_ACCESS_TOKEN")]
    _access_token: Option<String>,

    /// Web UI. Folder path.
    #[structopt(long, env = "RD_WEB_UI")]
    _web_ui: Option<String>,
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
    #[structopt(long, parse(from_os_str))]
    write_config: Option<PathBuf>,

    #[structopt(subcommand)]
    cmd: Option<Command>,
}

#[derive(StructOpt)]
enum Command {
    /// Generate schema to path, if not present, output to stdout
    GenerateSchema {
        #[structopt(parse(from_os_str))]
        path: Option<PathBuf>,
    },
    /// Run in server mode
    Server {
        #[structopt(flatten)]
        api_server: ApiServer,
    },
}

fn plugin_loader(_cfg: &rabbit_digger::Config, registry: &mut Registry) -> Result<()> {
    #[cfg(feature = "ss")]
    registry.init_with_registry("ss", ss::init)?;
    #[cfg(feature = "trojan")]
    registry.init_with_registry("trojan", trojan::init)?;
    Ok(())
}

async fn write_config(path: impl AsRef<Path>, cfg: &rabbit_digger::Config) -> Result<()> {
    let content = serde_yaml::to_string(cfg)?;
    tokio::fs::write(path, content.as_bytes()).await?;
    Ok(())
}

async fn run_api_server(controller: &controller::Controller, api_server: &ApiServer) -> Result<()> {
    if let Some(_bind) = &api_server.bind {
        #[cfg(feature = "api_server")]
        api_server::Server {
            controller: controller.clone(),
            access_token: api_server._access_token.to_owned(),
            web_ui: api_server._web_ui.to_owned(),
        }
        .run(_bind)
        .await
        .context("Failed to run api server.")?;
    }
    Ok(())
}

async fn real_main(args: Args) -> Result<()> {
    let controller = controller::Controller::new();
    controller.set_plugin_loader(plugin_loader).await;

    run_api_server(&controller, &args.api_server).await?;

    let config_path = args.config.clone();
    let config_stream = notify_stream(&config_path, RecursiveMode::Recursive)?;
    let write_config_path = args.write_config;

    let config_stream = stream::once(async { Ok(()) })
        .chain(
            config_stream
                .try_filter(|e| ready(e.kind.is_modify()))
                .map(|_| Ok(()))
                .debounce(Duration::from_millis(100)),
        )
        .and_then(move |_| read_to_string(config_path.clone()).map_err(Into::into))
        .and_then(|s| ready(serde_yaml::from_str(&s).map_err(Into::into)))
        .and_then(config::post_process)
        .and_then(|c: rabbit_digger::Config| async {
            if let Some(path) = &write_config_path {
                write_config(path, &c).await?;
            };
            Ok(c)
        });

    pin_mut!(config_stream);
    controller
        .run_stream(config_stream)
        .await
        .context("Failed to run RabbitDigger")?;

    Ok(())
}

#[paw::main]
#[tokio::main]
async fn main(args: Args) -> Result<()> {
    env_logger::Builder::from_env(
        Env::default()
            .default_filter_or("rabbit_digger=trace,rabbit_digger_pro=trace,rd_std=trace"),
    )
    .init();

    match &args.cmd {
        Some(Command::GenerateSchema { path }) => {
            if let Some(path) = path {
                schema::write_schema(path).await?;
            } else {
                let s = schema::generate_schema().await?;
                println!("{}", serde_json::to_string(&s)?);
            }
            return Ok(());
        }
        Some(Command::Server { api_server }) => {
            let controller = controller::Controller::new();

            run_api_server(&controller, &api_server).await?;

            return Ok(());
        }
        None => {}
    }

    match real_main(args).await {
        Ok(()) => {}
        Err(e) => log::error!("Process exit: {:?}", e),
    }

    Ok(())
}
