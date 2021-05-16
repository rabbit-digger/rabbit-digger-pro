use std::{convert::Infallible, future, path::PathBuf};

use super::{handlers, reject::handle_rejection, reject::ApiError, Server};
use rabbit_digger::controller::Controller;
use warp::{Filter, Rejection};

pub fn api(server: Server) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    let at = access_token(server.access_token);
    let prefix = warp::path!("api" / ..);

    prefix
        .and(at)
        .and(get_config(server.controller).recover(handle_rejection))
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

    return api(server).or(forward);
}

// GET /config
pub fn get_config(
    ctl: Controller,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("config")
        .and(warp::get())
        .and(with_ctl(ctl))
        .and_then(handlers::get_config)
}

fn with_ctl(ctl: Controller) -> impl Filter<Extract = (Controller,), Error = Infallible> + Clone {
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
