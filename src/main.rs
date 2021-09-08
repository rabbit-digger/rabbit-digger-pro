use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cfg_if::cfg_if;
use futures::{pin_mut, stream::TryStreamExt};
use rabbit_digger::{RabbitDigger, RabbitDiggerBuilder};
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

async fn run_api_server(rd: RabbitDigger, api_server: &ApiServer) -> Result<()> {
    if let Some(_bind) = &api_server.bind {
        #[cfg(feature = "api_server")]
        api_server::Server {
            rabbit_digger: rd,
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

async fn get_rd() -> Result<RabbitDigger> {
    let rd = RabbitDiggerBuilder::new()
        .plugin_loader(plugin_loader)
        .build()
        .await?;
    Ok(rd)
}

async fn real_main(args: Args) -> Result<()> {
    let rd = get_rd().await?;

    run_api_server(rd.clone(), &args.api_server).await?;

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
    rd.start_stream(config_stream)
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
            "rabbit_digger=debug,rabbit_digger_pro=debug,rd_std=debug,raw=debug",
        )
    }
    let filter = EnvFilter::from_default_env();

    let t = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter);

    cfg_if! {
        if #[cfg(feature = "console")] {
            let (layer, server) = console_subscriber::TasksLayer::new();
            tokio::spawn(server.serve());
            let filter = filter.add_directive("tokio=trace".parse()?);
            let t = t.with(layer);
        }
    }

    cfg_if! {
        if #[cfg(feature = "telemetry")] {
            let tracer = opentelemetry_jaeger::new_pipeline()
                .with_service_name("rabbit_digger_pro")
                .install_batch(opentelemetry::runtime::Tokio)?;
            // only for debug
            // let tracer = opentelemetry::sdk::export::trace::stdout::new_pipeline().install_simple();
            let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
            let t = t.with(opentelemetry);
        }
    }

    t.init();

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
            let rd = get_rd().await?;

            run_api_server(rd, &api_server).await?;

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
