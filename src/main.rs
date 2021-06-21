use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use controller::Controller;
use futures::{pin_mut, stream::TryStreamExt};
use rabbit_digger::controller;
#[cfg(feature = "api_server")]
use rabbit_digger_pro::api_server;
use rabbit_digger_pro::{plugin_loader, schema, watch_config_stream};
use structopt::StructOpt;

#[derive(StructOpt)]
struct ApiServer {
    /// HTTP endpoint bind address.
    #[structopt(short, long, env = "RD_BIND")]
    bind: Option<String>,

    /// Access token.
    #[structopt(long, env = "RD_ACCESS_TOKEN")]
    _access_token: Option<String>,

    /// Web UI. Folder path.
    #[structopt(long, env = "RD_WEB_UI")]
    _web_ui: Option<String>,

    /// Userdata.
    #[structopt(long, env = "RD_USERDATA", parse(from_os_str))]
    _userdata: Option<PathBuf>,
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

async fn write_config(path: impl AsRef<Path>, cfg: &rabbit_digger::Config) -> Result<()> {
    let content = serde_yaml::to_string(cfg)?;
    tokio::fs::write(path, content.as_bytes()).await?;
    Ok(())
}

async fn run_api_server(controller: &Controller, api_server: &ApiServer) -> Result<()> {
    if let Some(_bind) = &api_server.bind {
        #[cfg(feature = "api_server")]
        api_server::Server {
            controller: controller.clone(),
            access_token: api_server._access_token.to_owned(),
            web_ui: api_server._web_ui.to_owned(),
            userdata: api_server._userdata.to_owned(),
        }
        .run(_bind)
        .await
        .context("Failed to run api server.")?;
    }
    Ok(())
}

async fn real_main(args: Args) -> Result<()> {
    let controller = Controller::new();
    controller.set_plugin_loader(plugin_loader).await;

    run_api_server(&controller, &args.api_server).await?;

    let config_path = args.config.clone();
    let write_config_path = args.write_config;

    let config_stream =
        watch_config_stream(config_path)?.and_then(|c: rabbit_digger::Config| async {
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
    use tracing_subscriber::{layer::SubscriberExt, prelude::*, EnvFilter};
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var(
            "RUST_LOG",
            "rabbit_digger=trace,rabbit_digger_pro=trace,rd_std=trace,raw=trace",
        )
    }
    #[cfg(feature = "console")]
    let (layer, server) = console_subscriber::TasksLayer::new();
    let filter = EnvFilter::from_default_env();
    #[cfg(feature = "console")]
    let filter = filter.add_directive("tokio=trace".parse()?);

    let t = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter);
    #[cfg(feature = "console")]
    let t = t.with(layer);
    t.init();

    #[cfg(feature = "console")]
    tokio::spawn(server.serve());

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
            let controller = Controller::new();
            controller.set_plugin_loader(plugin_loader).await;

            run_api_server(&controller, &api_server).await?;

            tokio::signal::ctrl_c().await?;

            return Ok(());
        }
        None => {}
    }

    match real_main(args).await {
        Ok(()) => {}
        Err(e) => tracing::error!("Process exit: {:?}", e),
    }

    Ok(())
}
