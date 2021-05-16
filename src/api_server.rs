use anyhow::Result;
use rabbit_digger::controller::Controller;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;

mod filters;
mod handlers;
mod reject;

pub struct Server {
    pub access_token: Option<String>,
    pub controller: Controller,
}

impl Server {
    pub async fn run(self, bind: String) -> Result<()> {
        let routes = filters::api(self);
        let listener = TcpListener::bind(bind).await?;
        let listener = TcpListenerStream::new(listener);

        tokio::spawn(warp::serve(routes).run_incoming(listener));

        Ok(())
    }
}
