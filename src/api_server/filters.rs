use std::{convert::Infallible, future, path::PathBuf};

use super::{handlers, reject::handle_rejection, reject::ApiError, Server};
use rabbit_digger::controller::Controller;
use warp::{Filter, Rejection};

pub fn api(server: Server) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    let at = access_token(server.access_token);
    let prefix = warp::path!("api" / ..);
    // TODO: read or write userdata by API
    let _userdata = server
        .userdata
        .or(dirs::config_dir().map(|d| d.join("rabbit-digger")));
    let ctl = &server.controller;

    prefix.and(
        ws_event(&server.controller)
            .or(at.and(
                get_config(ctl)
                    .or(post_config(ctl))
                    .or(get_registry(ctl))
                    .or(get_state(ctl)),
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
        .allow_methods(["GET", "POST"]);

    return api(server).or(forward).with(cors);
}

// GET /config
pub fn get_config(
    ctl: &Controller,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("config")
        .and(warp::get())
        .and(with_ctl(ctl))
        .and_then(handlers::get_config)
}

// POST /config
pub fn post_config(
    ctl: &Controller,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("config")
        .and(warp::post())
        .and(with_ctl(ctl))
        .and(warp::body::json())
        .and_then(handlers::post_config)
}

// GET /registry
pub fn get_registry(
    ctl: &Controller,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("registry")
        .and(warp::get())
        .and(with_ctl(ctl))
        .and_then(handlers::get_registry)
}

// GET /state
pub fn get_state(
    ctl: &Controller,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("state")
        .and(warp::get())
        .and(with_ctl(ctl))
        .and_then(handlers::get_state)
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

fn access_token(
    access_token: Option<String>,
) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::optional::<String>("authorization")
        .and_then(move |header: Option<String>| {
            future::ready(if header == access_token {
                Ok(())
            } else {
                Err(warp::reject::custom(ApiError::Forbidden))
            })
        })
        .untuple_one()
}
