use std::{convert::Infallible, path::PathBuf};

use super::reject::{custom_reject, ApiError};
use futures::{pin_mut, SinkExt, Stream, TryStreamExt};
use rabbit_digger::RabbitDigger;
use serde_json::json;
use tokio::fs::{create_dir_all, read_to_string, remove_file, File};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
use tokio_util::codec::{BytesCodec, FramedWrite};
use warp::{
    path::Tail,
    ws::{Message, WebSocket},
    Buf,
};

use crate::config::ConfigExt;

pub async fn get_config(rd: RabbitDigger) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(
        &rd.config()
            .await
            .map_err(|_| warp::reject::custom(ApiError::NotFound))?,
    ))
}

pub async fn post_config(
    rd: RabbitDigger,
    config: ConfigExt,
) -> Result<impl warp::Reply, warp::Rejection> {
    let config = config.post_process().await.map_err(custom_reject)?;
    let reply = warp::reply::json(&config);

    rd.stop().await.map_err(custom_reject)?;
    rd.start(config).await.map_err(custom_reject)?;

    Ok(reply)
}

pub async fn get_registry(rd: RabbitDigger) -> Result<impl warp::Reply, Infallible> {
    Ok(rd.registry(|r| warp::reply::json(&r)).await)
}

pub async fn get_connection(rd: RabbitDigger) -> Result<impl warp::Reply, Infallible> {
    Ok(rd.connection(|c| warp::reply::json(&c)).await)
}

pub async fn get_state(rd: RabbitDigger) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(
        &rd.state_str()
            .await
            .map_err(|_| warp::reject::custom(ApiError::NotFound))?,
    ))
}

pub async fn get_userdata(
    mut userdata: PathBuf,
    tail: Tail,
) -> Result<impl warp::Reply, warp::Rejection> {
    // TOOD prevent ".." attack
    userdata.push(tail.as_str());
    let body = read_to_string(userdata)
        .await
        .map_err(|_| warp::reject::custom(ApiError::NotFound))?;
    Ok(warp::reply::json(&json!({ "body": body })))
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
    Ok(warp::reply::json(&json!({ "copied": size })))
}

pub async fn delete_userdata(
    mut userdata: PathBuf,
    tail: Tail,
) -> Result<impl warp::Reply, warp::Rejection> {
    create_dir_all(&userdata).await.map_err(custom_reject)?;
    // TOOD prevent ".." attack
    userdata.push(tail.as_str());

    remove_file(&userdata).await.map_err(custom_reject)?;

    Ok(warp::reply::json(&json!({ "ok": true })))
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

pub async fn ws_event(rd: RabbitDigger, ws: warp::ws::Ws) -> Result<impl warp::Reply, Infallible> {
    let sub = BroadcastStream::new(rd.get_subscriber().await);
    let sub = sub.map_ok(|i| {
        let events = serde_json::to_string(&i).unwrap();
        Message::text(events)
    });
    Ok(ws.on_upgrade(move |ws| async move {
        if let Err(e) = forward(sub, ws).await {
            tracing::error!("WebSocket event error: {:?}", e)
        }
    }))
}
