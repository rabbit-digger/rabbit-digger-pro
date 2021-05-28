use std::convert::Infallible;

use super::reject::custom_reject;
use futures::{SinkExt, Stream, TryStreamExt};
use rabbit_digger::controller::{Controller, OnceConfigStopper};
use rd_interface::Arc;
use tokio::sync::Mutex;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
use warp::ws::{Message, WebSocket};

use crate::config::ConfigExt;

pub async fn get_config(ctl: Controller) -> Result<impl warp::Reply, Infallible> {
    let ctl = ctl.lock().await;

    Ok(warp::reply::json(&ctl.config()))
}

pub async fn post_config(
    ctl: Controller,
    config: ConfigExt,
    last_stopper: Arc<Mutex<Option<OnceConfigStopper>>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let config = config.post_process().await.map_err(custom_reject)?;
    let reply = warp::reply::json(&config);

    let mut last_stopper = last_stopper.lock().await;
    if let Some(stopper) = last_stopper.take() {
        stopper.stop().await.map_err(custom_reject)?;
    }
    *last_stopper = Some(ctl.start(config).await);

    Ok(reply)
}

pub async fn get_registry(ctl: Controller) -> Result<impl warp::Reply, Infallible> {
    let ctl = ctl.lock().await;

    Ok(warp::reply::json(&ctl.registry()))
}

pub async fn get_state(ctl: Controller) -> Result<impl warp::Reply, Infallible> {
    let ctl = ctl.lock().await;

    Ok(warp::reply::json(&ctl.state()))
}

async fn forward(
    mut sub: impl Stream<Item = Result<Message, BroadcastStreamRecvError>> + Unpin,
    mut ws: WebSocket,
) -> anyhow::Result<()> {
    while let Some(item) = sub.try_next().await? {
        ws.send(item).await?;
    }
    Ok(())
}

pub async fn ws_event(ctl: Controller, ws: warp::ws::Ws) -> Result<impl warp::Reply, Infallible> {
    let sub = BroadcastStream::new(ctl.get_subscriber().await);
    let sub = sub.map_ok(|i| {
        Message::text(
            i.into_iter()
                .map(|e| format!("{:?}", e))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    });
    Ok(ws.on_upgrade(move |ws| async move {
        if let Err(e) = forward(sub, ws).await {
            tracing::error!("WebSocket event error: {:?}", e)
        }
    }))
}
