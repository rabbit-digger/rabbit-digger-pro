mod config;
mod plugins;
mod rule;

use std::{collections::HashMap, fs::File};
use std::{fmt, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use env_logger::Env;
use futures::{prelude::*, stream::FuturesUnordered};
use plugins::load_plugins;
use rd_interface::{config::Value, Arc, Net, NoopNet, Registry, Server};
use rule::Rule;
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

fn get_local(registry: &Registry, noop: Net) -> Result<Net> {
    let builder = registry
        .net
        .get("local")
        .ok_or(anyhow!("Failed to get local net"))?;
    let local = builder(noop, Value::Null)?;
    Ok(local)
}

async fn start_server(server: Server) -> Result<()> {
    server.start().await?;
    Ok(())
}

struct ServerInfo {
    name: String,
    listen: String,
    net: String,
    server: Server,
    config: Value,
}

impl fmt::Debug for ServerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerInfo")
            .field("name", &self.name)
            .field("listen", &self.listen)
            .field("net", &self.net)
            .field("config", &self.config)
            .finish()
    }
}

async fn real_main(args: Args) -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("rabbit_digger=debug")).init();
    let config: config::Config =
        serde_yaml::from_reader(File::open(args.config).context("Failed to open config file.")?)?;

    let registry = load_plugins(config.plugin_path)?;
    log::debug!("registry: {:?}", registry);

    let mut net: HashMap<String, Net> = HashMap::new();
    let mut servers: Vec<ServerInfo> = Vec::new();
    let noop = Arc::new(NoopNet);

    net.insert("noop".to_string(), noop.clone());
    net.insert("local".to_string(), get_local(&registry, noop)?);

    for i in config.net {
        let builder = registry
            .net
            .get(&i.net_type)
            .ok_or(anyhow!("Proxy type is not loaded: {}", &i.net_type))?;
        let chain = net.get(&i.chain).ok_or(anyhow!(
            "Chain {} is not loaded. Required by {}",
            &i.chain,
            &i.name
        ))?;
        let proxy = builder(chain.clone(), i.rest).context(format!(
            "Failed to build net {:?}. Please check your config.",
            i.name
        ))?;
        net.insert(i.name, proxy);
    }

    net.insert("rule".to_string(), Rule::new(net.clone()));

    for i in config.server {
        let builder = registry
            .server
            .get(&i.server_type)
            .ok_or(anyhow!("Server type is not loaded: {}", &i.server_type))?;
        let listen = net.get(&i.listen).ok_or(anyhow!(
            "Listen Net {} is not loaded. Required by {:?}",
            &i.net,
            &i.name
        ))?;
        let net = net.get(&i.net).ok_or(anyhow!(
            "Net {} is not loaded. Required by {:?}",
            &i.net,
            &i.name
        ))?;
        let server = builder(listen.clone(), net.clone(), i.rest.clone()).context(format!(
            "Failed to build server {:?}. Please check your config.",
            i.name
        ))?;
        servers.push(ServerInfo {
            name: i.name,
            server,
            config: i.rest,
            listen: i.listen,
            net: i.net,
        });
    }
    log::info!("proxy {:#?}", net.keys());
    log::info!("server {:#?}", servers);

    let mut tasks: FuturesUnordered<_> = servers
        .into_iter()
        .map(|i| start_server(i.server))
        .collect();

    while tasks.next().await.is_some() {}

    log::info!("all servers are down, exit.");

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
