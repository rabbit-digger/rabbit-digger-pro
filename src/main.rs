use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cfg_if::cfg_if;
use futures::{pin_mut, stream::TryStreamExt};
use rabbit_digger::{RabbitDigger, RabbitDiggerBuilder};
#[cfg(feature = "api_server")]
use rabbit_digger_pro::api_server;
use rabbit_digger_pro::{
    config::{ConfigManager, ImportSource},
    plugin_loader, schema,
};
use structopt::StructOpt;
use tracing_subscriber::filter::dynamic_filter_fn;

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

async fn run_api_server(
    _rd: RabbitDigger,
    _cfg_mgr: ConfigManager,
    api_server: &ApiServer,
) -> Result<()> {
    if let Some(_bind) = &api_server.bind {
        #[cfg(feature = "api_server")]
        api_server::Server {
            rabbit_digger: _rd,
            config_manager: _cfg_mgr,
            access_token: api_server._access_token.to_owned(),
            web_ui: api_server._web_ui.to_owned(),
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

async fn get_cfg() -> Result<ConfigManager> {
    Ok(ConfigManager::new().await?)
}

async fn real_main(args: Args) -> Result<()> {
    let cfg_mgr = get_cfg().await?;
    let rd = get_rd().await?;

    run_api_server(rd.clone(), cfg_mgr.clone(), &args.api_server).await?;

    let config_path = args.config.clone();
    let write_config_path = args.write_config;

    let config_stream = cfg_mgr
        .config_stream(ImportSource::Path(config_path))
        .await?
        .and_then(|c: rabbit_digger::Config| async {
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
            "rabbit_digger=debug,rabbit_digger_pro=debug,rd_std=debug,raw=debug,ss=debug",
        )
    }
    let tr = tracing_subscriber::registry();

    cfg_if! {
        if #[cfg(feature = "console")] {
            let (layer, server) = console_subscriber::TasksLayer::builder().with_default_env().build();
            tokio::spawn(server.serve());
            let tr = tr.with(layer);
        }
    }

    cfg_if! {
        if #[cfg(feature = "telemetry")] {
            let tracer = opentelemetry_jaeger::new_pipeline()
                .with_service_name("rabbit_digger_pro")
                .install_batch(opentelemetry::runtime::Tokio)?;
            // only for debug
            // let tracer = opentelemetry::sdk::export::trace::stdout::new_pipeline().install_simple();
            let tracer_filter =
                EnvFilter::new("rabbit_digger=trace,rabbit_digger_pro=trace,rd_std=trace");
            let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
            let tr = tr.with(
                opentelemetry.with_filter(dynamic_filter_fn(move |metadata, ctx| {
                    tracer_filter.enabled(metadata, ctx.clone())
                })),
            );
        }
    }

    let log_filter = EnvFilter::from_default_env();
    let log_writer_filter =
        EnvFilter::new("rabbit_digger=debug,rabbit_digger_pro=debug,rd_std=debug,raw=debug");
    tr.with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout)
            .with_filter(dynamic_filter_fn(move |metadata, ctx| {
                log_filter.enabled(metadata, ctx.clone())
            })),
    )
    .with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(rabbit_digger_pro::log::LogWriter::new)
            .with_filter(dynamic_filter_fn(move |metadata, ctx| {
                log_writer_filter.enabled(metadata, ctx.clone())
            })),
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
            let rd = get_rd().await?;
            let cfg_mgr = get_cfg().await?;

            run_api_server(rd, cfg_mgr, &api_server).await?;

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

#[cfg(feature = "jemalloc")]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
