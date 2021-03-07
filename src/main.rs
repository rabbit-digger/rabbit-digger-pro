mod config;
mod plugins;
mod registry;
mod rule;

use std::{collections::HashMap, fs::File};
use std::{fmt, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use env_logger::Env;
use futures::{prelude::*, stream::FuturesUnordered};
use plugins::load_plugins;
use rd_interface::{config::Value, Arc, Net, NotImplementedNet, Server};
use registry::Registry;
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
    let net_item = &registry.get_net("local")?;
    let local = net_item.build(noop, Value::Null)?;
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

fn init_net(
    registry: &Registry,
    config: config::ConfigNet,
    rule_config: config::ConfigRule,
) -> Result<HashMap<String, Net>> {
    let mut net: HashMap<String, Net> = HashMap::new();
    let noop = Arc::new(NotImplementedNet);

    net.insert("noop".to_string(), noop.clone());
    net.insert("local".to_string(), get_local(&registry, noop)?);

    for i in config {
        let net_item = registry.get_net(&i.net_type)?;
        let chain = net.get(&i.chain).ok_or(anyhow!(
            "Chain {} is not loaded. Required by {}",
            &i.chain,
            &i.name
        ))?;
        let proxy = net_item.build(chain.clone(), i.rest).context(format!(
            "Failed to build net {:?}. Please check your config.",
            i.name
        ))?;
        net.insert(i.name, proxy);
    }

    net.insert("rule".to_string(), Rule::new(net.clone(), rule_config)?);

    Ok(net)
}

fn init_server(
    registry: &Registry,
    net: &HashMap<String, Net>,
    config: config::ConfigServer,
) -> Result<Vec<ServerInfo>> {
    let mut servers: Vec<ServerInfo> = Vec::new();

    for i in config {
        let server_item = registry.get_server(&i.server_type)?;
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
        let server = server_item
            .build(listen.clone(), net.clone(), i.rest.clone())
            .context(format!(
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

    Ok(servers)
}

async fn real_main(args: Args) -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("rabbit_digger=trace")).init();
    let config: config::Config =
        serde_yaml::from_reader(File::open(args.config).context("Failed to open config file.")?)?;

    let registry = load_plugins(config.plugin_path)?;
    log::debug!("registry: {:?}", registry);

    let net = init_net(&registry, config.net, config.rule)?;
    let servers = init_server(&registry, &net, config.server)?;

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
