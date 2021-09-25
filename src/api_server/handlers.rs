use std::{
    convert::Infallible,
    error::Error,
    future::ready,
    str::from_utf8,
    sync::Arc,
    time::{Duration, Instant},
};

use super::reject::{custom_reject, ApiError};
use bytes::Bytes;
use futures::{SinkExt, Stream, StreamExt, TryStreamExt};
use rabbit_digger::{RabbitDigger, Uuid};
use rd_interface::{IntoAddress, Value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{pin, time::interval};
use tokio_stream::wrappers::IntervalStream;
use warp::{
    path::Tail,
    ws::{Message, WebSocket},
};

use crate::{
    config::{ConfigManager, ImportSource, SelectMap},
    storage::{FileStorage, Storage},
};

#[derive(Clone)]
pub(super) struct Ctx {
    pub(super) rd: RabbitDigger,
    pub(super) cfg_mgr: ConfigManager,
    pub(super) userdata: Arc<FileStorage>,
}

pub(super) async fn get_config(Ctx { rd, .. }: Ctx) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(
        &rd.config()
            .await
            .map_err(|_| warp::reject::custom(ApiError::NotFound))?,
    ))
}

pub(super) async fn post_config(
    Ctx { rd, cfg_mgr, .. }: Ctx,
    source: ImportSource,
) -> Result<impl warp::Reply, warp::Rejection> {
    let stream = cfg_mgr.config_stream(source).await.map_err(custom_reject)?;

    let reply = warp::reply::json(&Value::Null);

    rd.stop().await.map_err(custom_reject)?;
    rd.start_stream(stream).await.map_err(custom_reject)?;

    Ok(reply)
}

pub(super) async fn get_registry(Ctx { rd, .. }: Ctx) -> Result<impl warp::Reply, Infallible> {
    Ok(rd.registry(|r| warp::reply::json(&r)).await)
}

pub(super) async fn get_connection(Ctx { rd, .. }: Ctx) -> Result<impl warp::Reply, Infallible> {
    Ok(rd.connection(|c| warp::reply::json(&c)).await)
}

pub(super) async fn get_state(Ctx { rd, .. }: Ctx) -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(
        &rd.state_str()
            .await
            .map_err(|_| warp::reject::custom(ApiError::NotFound))?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct PostSelect {
    selected: String,
}
pub(super) async fn post_select(
    Ctx { rd, cfg_mgr, .. }: Ctx,
    net_name: String,
    PostSelect { selected }: PostSelect,
) -> Result<impl warp::Reply, warp::Rejection> {
    let net_name = percent_encoding::percent_decode(net_name.as_bytes())
        .decode_utf8()
        .map_err(custom_reject)?;
    rd.update_net(&net_name, |o| {
        if o.net_type == "select" {
            if let Some(o) = o.opt.as_object_mut() {
                o.insert("selected".to_string(), selected.clone().into());
            }
        } else {
            tracing::warn!("net_type is not select");
        }
    })
    .await
    .map_err(custom_reject)?;

    if let Some(id) = rd.get_config(|c| c.map(|c| c.id.clone())).await {
        let mut select_map = SelectMap::from_cache(&id, cfg_mgr.select_storage())
            .await
            .map_err(custom_reject)?;

        select_map.insert(net_name.to_string(), selected);

        select_map
            .write_cache(&id, cfg_mgr.select_storage())
            .await
            .map_err(custom_reject)?;
    }

    Ok(warp::reply::json(&Value::Null))
}

pub(super) async fn delete_conn(
    Ctx { rd, .. }: Ctx,
    uuid: Uuid,
) -> Result<impl warp::Reply, warp::Rejection> {
    let ok = rd.stop_connection(uuid).await.map_err(custom_reject)?;
    Ok(warp::reply::json(&ok))
}

#[derive(Debug, Deserialize)]
pub struct DelayRequest {
    url: url::Url,
    timeout: Option<u64>,
}
#[derive(Debug, Serialize)]
pub struct DelayResponse {
    connect: u64,
    response: u64,
}
pub(super) async fn get_delay(
    Ctx { rd, .. }: Ctx,
    net_name: String,
    DelayRequest { url, timeout }: DelayRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let net_name = percent_encoding::percent_decode(net_name.as_bytes())
        .decode_utf8()
        .map_err(custom_reject)?;
    let net = rd
        .get_net(&net_name)
        .await
        .map_err(custom_reject)?
        .map(|n| n.net());
    let host = url.host_str();
    let port = url.port_or_known_default();
    let timeout = timeout.unwrap_or(5000);
    Ok(match (net, host, port) {
        (Some(net), Some(host), Some(port)) => {
            let start = Instant::now();
            let fut = async {
                let socket = net
                    .tcp_connect(
                        &mut rd_interface::Context::new(),
                        &(host, port).into_address()?,
                    )
                    .await?;
                let connect = start.elapsed().as_millis() as u64;
                let (mut request_sender, connection) =
                    hyper::client::conn::handshake(socket).await?;
                let connect_req = hyper::Request::builder()
                    .method("GET")
                    .uri(url.path())
                    .body(hyper::Body::empty())?;
                tokio::spawn(connection);
                let _connect_resp = request_sender.send_request(connect_req).await?;
                let response = start.elapsed().as_millis() as u64;
                anyhow::Result::<DelayResponse>::Ok(DelayResponse { connect, response })
            };
            let resp = tokio::time::timeout(Duration::from_millis(timeout), fut).await;
            let resp = match resp {
                Ok(v) => Some(v.map_err(custom_reject)?),
                _ => None,
            };
            warp::reply::json(&resp)
        }
        _ => warp::reply::json(&Value::Null),
    })
}

pub(super) async fn get_userdata(
    Ctx { userdata, .. }: Ctx,
    tail: Tail,
) -> Result<impl warp::Reply, warp::Rejection> {
    let item = userdata
        .get(tail.as_str())
        .await
        .map_err(|_| warp::reject::custom(ApiError::NotFound))?
        .ok_or_else(|| warp::reject::custom(ApiError::NotFound))?;

    Ok(warp::reply::json(&item))
}

pub(super) async fn put_userdata(
    Ctx { userdata, .. }: Ctx,
    tail: Tail,
    body: Bytes,
) -> Result<impl warp::Reply, warp::Rejection> {
    userdata
        .set(tail.as_str(), from_utf8(&body).map_err(custom_reject)?)
        .await
        .map_err(custom_reject)?;

    Ok(warp::reply::json(&json!({ "copied": body.len() })))
}

pub(super) async fn delete_userdata(
    Ctx { userdata, .. }: Ctx,
    tail: Tail,
) -> Result<impl warp::Reply, warp::Rejection> {
    userdata
        .remove(tail.as_str())
        .await
        .map_err(custom_reject)?;

    Ok(warp::reply::json(&json!({ "ok": true })))
}

async fn forward<E>(
    sub: impl Stream<Item = Result<Message, E>>,
    mut ws: WebSocket,
) -> anyhow::Result<()>
where
    E: Error + Send + Sync + 'static,
{
    pin!(sub);
    while let Some(item) = sub.try_next().await? {
        ws.send(item).await?;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct ConnectionQuery {
    #[serde(default)]
    pub patch: bool,
    #[serde(default)]
    pub without_connections: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MaybePatch {
    Full(Value),
    Patch(json_patch::Patch),
}

pub(super) async fn ws_conn(
    Ctx { rd, .. }: Ctx,
    query: ConnectionQuery,
    ws: warp::ws::Ws,
) -> Result<impl warp::Reply, Infallible> {
    let ConnectionQuery {
        patch: patch_mode,
        without_connections,
    } = query;
    let stream = IntervalStream::new(interval(Duration::from_secs(1)));
    let stream = stream
        .then(move |_| {
            let rd = rd.clone();
            async move { rd.connection(|c| serde_json::to_value(c)).await }
        })
        .map_ok(move |mut v| {
            if let (Some(o), true) = (v.as_object_mut(), without_connections) {
                o.remove("connections");
            }
            v
        })
        .scan(Option::<Value>::None, move |last, r| {
            ready(Some(match (patch_mode, r) {
                (true, Ok(x)) => {
                    let r = if let Some(lv) = last {
                        MaybePatch::Patch(json_patch::diff(lv, &x))
                    } else {
                        MaybePatch::Full(x.clone())
                    };
                    *last = Some(x);
                    Ok(r)
                }
                (_, Ok(x)) => Ok(MaybePatch::Full(x)),
                (_, Err(e)) => Err(e),
            }))
        })
        .map_ok(|p| Message::text(serde_json::to_string(&p).unwrap()));
    Ok(ws.on_upgrade(move |ws| async move {
        if let Err(e) = forward(stream, ws).await {
            tracing::error!("WebSocket event error: {:?}", e)
        }
    }))
}

pub(super) async fn ws_log(ws: warp::ws::Ws) -> Result<impl warp::Reply, Infallible> {
    Ok(ws.on_upgrade(move |mut ws| async move {
        let mut recv = crate::log::get_sender().subscribe();
        while let Ok(content) = recv.recv().await {
            if let Err(e) = ws
                .send(Message::text(String::from_utf8_lossy(&content)))
                .await
            {
                tracing::error!("WebSocket event error: {:?}", e);
                break;
            }
        }
    }))
}
