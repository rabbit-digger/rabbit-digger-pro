use std::{convert::Infallible, future, path::PathBuf};

use super::{handlers, reject::handle_rejection, reject::ApiError, Server};
use rabbit_digger::controller::{Controller, OnceConfigStopper};
use rd_interface::Arc;
use tokio::sync::Mutex;
use warp::{Filter, Rejection};

pub fn api(server: Server) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    let at = check_access_token(server.access_token);
    let prefix = warp::path!("api" / ..);
    // TODO: read or write userdata by API
    let userdata = &server
        .userdata
        .or(dirs::config_dir().map(|d| d.join("rabbit-digger")));
    let ctl = &server.controller;
    let stopper: Arc<Mutex<Option<OnceConfigStopper>>> = Arc::new(Mutex::new(None));

    let get_config = warp::path!("config")
        .and(warp::get())
        .and(with_ctl(ctl))
        .and_then(handlers::get_config);
    let post_config = warp::path!("config")
        .and(warp::post())
        .and(with_ctl(ctl))
        .and(warp::body::json())
        .and(warp::any().map(move || stopper.clone()))
        .and_then(handlers::post_config);
    let get_registry = warp::path!("registry")
        .and(warp::get())
        .and(with_ctl(ctl))
        .and_then(handlers::get_registry);
    let get_state = warp::path!("state")
        .and(warp::get())
        .and(with_ctl(ctl))
        .and_then(handlers::get_state);

    let get_userdata = warp::path("userdata")
        .and(warp::get())
        .and(with_userdata(userdata))
        .and(warp::path::tail())
        .and_then(handlers::get_userdata);
    let put_userdata = warp::path("userdata")
        .and(warp::put())
        .and(with_userdata(userdata))
        .and(warp::path::tail())
        .and(warp::body::stream())
        .and_then(handlers::put_userdata);

    prefix.and(
        ws_event(&server.controller)
            .or(at.and(
                get_config
                    .or(post_config)
                    .or(get_registry)
                    .or(get_state)
                    .or(get_userdata)
                    .or(put_userdata),
            ))
            .recover(handle_rejection),
    )
}

pub fn routes(
    server: Server,
) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
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
        .allow_methods(["GET", "POST", "PUT"]);

    return api(server).or(forward).with(cors);
}

// Websocket /event
pub fn ws_event(
    ctl: &Controller,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("event")
        .and(with_ctl(ctl))
        .and(warp::ws())
        .and_then(handlers::ws_event)
}

fn with_ctl(ctl: &Controller) -> impl Filter<Extract = (Controller,), Error = Infallible> + Clone {
    let ctl = ctl.clone();
    warp::any().map(move || ctl.clone())
}

fn with_userdata(
    userdata: &Option<PathBuf>,
) -> impl Filter<Extract = (PathBuf,), Error = Rejection> + Clone {
    let userdata = userdata.clone();
    if let Some(userdata) = userdata {
        warp::any().map(move || userdata.clone()).boxed()
    } else {
        warp::any()
            .and_then(|| future::ready(Err(warp::reject::custom(ApiError::Forbidden))))
            .boxed()
    }
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
