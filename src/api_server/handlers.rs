use std::{convert::Infallible, path::PathBuf};

use super::reject::{custom_reject, ApiError};
use futures::{pin_mut, SinkExt, Stream, TryStreamExt};
use rabbit_digger::controller::{Controller, OnceConfigStopper};
use rd_interface::Arc;
use serde_json::json;
use tokio::{
    fs::{create_dir_all, File},
    sync::Mutex,
};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};
use warp::{
    hyper::Response,
    path::Tail,
    ws::{Message, WebSocket},
    Buf,
};

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

pub async fn get_userdata(
    mut userdata: PathBuf,
    tail: Tail,
) -> Result<impl warp::Reply, warp::Rejection> {
    // TOOD prevent ".." attack
    userdata.push(tail.as_str());
    let file = File::open(userdata)
        .await
        .map_err(|_| warp::reject::custom(ApiError::NotFound))?;
    let stream = FramedRead::new(file, BytesCodec::new());
    let resp = Response::builder()
        .body(warp::hyper::body::Body::wrap_stream(stream))
        .map_err(custom_reject)?;
    Ok(resp)
}

pub async fn put_userdata(
    mut userdata: PathBuf,
    tail: Tail,
    body: impl Stream<Item = Result<impl Buf, warp::Error>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    create_dir_all(&userdata).await.map_err(custom_reject)?;
    // TOOD prevent ".." attack
    userdata.push(tail.as_str());
    let file = File::create(&userdata).await.map_err(custom_reject)?;
    let mut stream = FramedWrite::new(file, BytesCodec::new());
    let mut size = 0;
    pin_mut!(body);
    while let Some(mut chunk) = body.try_next().await.map_err(custom_reject)? {
        let len = chunk.remaining();
        size += len;
        stream
            .send(chunk.copy_to_bytes(len))
            .await
            .map_err(custom_reject)?;
    }
    Ok(warp::reply::json(&json!({"ok": true, "copied": size})))
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
