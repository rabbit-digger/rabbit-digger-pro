use std::{convert::Infallible, future, path::PathBuf, sync::Arc};

use crate::storage::FileStorage;
use anyhow::Result;

use super::{
    handlers::{self, Ctx},
    reject::handle_rejection,
    reject::ApiError,
    Server,
};
use warp::{Filter, Rejection};

pub async fn api(
    server: Server,
) -> Result<impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone> {
    let at = check_access_token(server.access_token);
    let prefix = warp::path!("api" / ..);
    // TODO: read or write userdata by API
    let ctx = Ctx {
        rd: server.rabbit_digger.clone(),
        cfg_mgr: server.config_manager.clone(),
        userdata: Arc::new(FileStorage::new("userdata").await?),
    };

    let get_config = warp::path!("config")
        .and(warp::get())
        .and(with_ctx(&ctx))
        .and_then(handlers::get_config);
    let post_config = warp::path!("config")
        .and(warp::post())
        .and(with_ctx(&ctx))
        .and(warp::body::json())
        .and_then(handlers::post_config);
    let get_registry = warp::path!("registry")
        .and(warp::get())
        .and(with_ctx(&ctx))
        .and_then(handlers::get_registry);
    let get_connection = warp::path!("connection")
        .and(warp::get())
        .and(with_ctx(&ctx))
        .and_then(handlers::get_connection);
    let get_state = warp::path!("state")
        .and(warp::get())
        .and(with_ctx(&ctx))
        .and_then(handlers::get_state);
    let post_select = warp::path("net")
        .and(warp::post())
        .and(with_ctx(&ctx))
        .and(warp::path::param())
        .and(warp::body::json())
        .and_then(handlers::post_select);
    let get_delay = warp::path("net")
        .and(warp::get())
        .and(with_ctx(&ctx))
        .and(warp::path::param())
        .and(warp::path("delay"))
        .and(warp::query::<handlers::DelayRequest>())
        .and_then(handlers::get_delay);
    let delete_conn = warp::path("connection")
        .and(warp::delete())
        .and(with_ctx(&ctx))
        .and(warp::path::param())
        .and_then(handlers::delete_conn);

    let get_userdata = warp::path("userdata")
        .and(warp::get())
        .and(with_ctx(&ctx))
        .and(warp::path::tail())
        .and_then(handlers::get_userdata);
    let put_userdata = warp::path("userdata")
        .and(warp::put())
        .and(with_ctx(&ctx))
        .and(warp::path::tail())
        .and(warp::body::bytes())
        .and_then(handlers::put_userdata);
    let delete_userdata = warp::path("userdata")
        .and(warp::delete())
        .and(with_ctx(&ctx))
        .and(warp::path::tail())
        .and_then(handlers::delete_userdata);

    Ok(prefix.and(
        ws_connection(&ctx)
            .or(ws_log())
            .or(at.and(
                get_config
                    .or(post_config)
                    .or(get_registry)
                    .or(get_connection)
                    .or(get_state)
                    .or(post_select)
                    .or(delete_conn)
                    .or(get_delay)
                    .or(get_userdata)
                    .or(put_userdata)
                    .or(delete_userdata),
            ))
            .recover(handle_rejection),
    ))
}

pub async fn routes(
    server: Server,
) -> Result<impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone> {
    let web_ui = server.web_ui.clone();
    let forward = match web_ui {
        Some(web_ui) => warp::get()
            .and(warp::fs::dir(web_ui.clone()))
            .or(warp::fs::file(PathBuf::from(web_ui).join("index.html")))
            .boxed(),
        None => warp::any()
            .and_then(|| future::ready(Err(warp::reject::not_found())))
            .boxed(),
    };

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(["authorization", "content-type"])
        .allow_methods(["GET", "POST", "PUT", "DELETE"]);

    Ok(api(server).await?.or(forward).with(cors))
}

// Websocket /connection
fn ws_connection(
    ctx: &Ctx,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("connection")
        .and(with_ctx(ctx))
        .and(warp::query::<handlers::ConnectionQuery>())
        .and(warp::ws())
        .and_then(handlers::ws_conn)
}

// Websocket /log
pub fn ws_log() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("log")
        .and(warp::ws())
        .and_then(handlers::ws_log)
}

fn with_ctx(ctx: &Ctx) -> impl Filter<Extract = (Ctx,), Error = Infallible> + Clone {
    let ctx = ctx.clone();
    warp::any().map(move || ctx.clone())
}

fn check_access_token(
    access_token: Option<String>,
) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::optional::<String>("authorization")
        .and_then(move |header: Option<String>| {
            future::ready(match &access_token {
                Some(token) if token == &header.unwrap_or_default() => Ok(()),
                None => Ok(()),
                _ => Err(warp::reject::custom(ApiError::Forbidden)),
            })
        })
        .untuple_one()
}
