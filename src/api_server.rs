use std::net::SocketAddr;

use anyhow::Result;
use rabbit_digger::RabbitDigger;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;

use crate::{config::ConfigManager, storage::FileStorage};

mod filters;
mod handlers;
mod reject;

pub struct Server {
    pub rabbit_digger: RabbitDigger,
    pub config_manager: ConfigManager,
    pub access_token: Option<String>,
    pub web_ui: Option<String>,
    pub userdata: FileStorage,
}

impl Server {
    pub async fn run(self, bind: &str) -> Result<SocketAddr> {
        let routes = filters::routes(self).await;
        let listener = TcpListener::bind(bind).await?;
        let local_addr = listener.local_addr()?;
        let listener = TcpListenerStream::new(listener);

        tokio::spawn(warp::serve(routes).run_incoming(listener));

        Ok(local_addr)
    }
}
