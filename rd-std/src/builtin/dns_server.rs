use rd_interface::{
    async_trait, config::NetRef, prelude::*, registry::Builder, Address, IServer, Net, Result,
    Server,
};
use std::net::{IpAddr, Ipv4Addr};
use trust_dns_resolver::config::NameServerConfig;
use trust_dns_server::authority::{Authority, Catalog};
use trust_dns_server::server::{RequestHandler, ServerFuture};
use trust_dns_server::store::forwarder::{ForwardAuthority, ForwardConfig};

/// A DNS server.
#[rd_config]
#[derive(Debug)]
pub struct DnsServerConfig {
    #[serde(default)]
    listen_net: NetRef,
    #[serde(default)]
    net: NetRef,
    #[serde()]
    bind: Address,
}

pub struct DnsServer {
    listen_net: Net,
    net: Net,
    bind: Address,
}

#[async_trait]
impl IServer for DnsServer {
    async fn start(&self) -> Result<()> {
        fn main() {
            // 创建一个转发配置，指定上游的解析器地址和协议
            let forward_config = ForwardConfig {
                name_servers: vec![NameServerConfig {
                    socket_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
                    protocol: Protocol::Udp,
                    tls_dns_name: None,
                }],
                ..Default::default()
            };

            // 创建一个转发域名记录
            let forward_authority = ForwardAuthority::try_from_config(
                Name::from_str(".").unwrap(), // 转发所有域名
                ZoneType::Forward,
                &forward_config,
            )
            .unwrap();

            // 创建一个目录，用于存放所有的域名记录
            let mut catalog = Catalog::new();
            catalog.upsert(forward_authority.origin().clone(), forward_authority);

            // 创建一个请求处理器，用于处理DNS请求和响应
            let request_handler = RequestHandler::new(catalog);

            // 创建一个服务器对象，用于监听和接收DNS请求
            let mut server = ServerFuture::new(request_handler);

            // 绑定到本地地址和端口
            server.register_socket_std(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 53);

            // 使用tokio运行时启动并运行服务器
            tokio_runtime.block_on(server.block_until_done());
        }
        todo!()
    }
}

impl Builder<Server> for DnsServer {
    const NAME: &'static str = "dns";
    type Config = DnsServerConfig;
    type Item = Self;

    fn build(config: Self::Config) -> Result<Self> {
        let listen_net = config.listen_net.value_cloned();
        let net = config.net.value_cloned();
        let bind = config.bind;

        Ok(Self {
            listen_net,
            net,
            bind,
        })
    }
}
