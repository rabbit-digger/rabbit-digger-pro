use anyhow::Result;
use rabbit_digger::controller::Controller;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;

mod filters;
mod handlers;

pub async fn run(bind: String, controller: Controller) -> Result<()> {
    let routes = filters::api(controller);
    let listener = TcpListener::bind(bind).await?;
    let listener = TcpListenerStream::new(listener);

    tokio::spawn(warp::serve(routes).run_incoming(listener));

    Ok(())
}
