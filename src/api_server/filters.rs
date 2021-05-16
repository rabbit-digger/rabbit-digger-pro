use std::{convert::Infallible, future};

use super::{
    handlers,
    reject::ApiError,
    reject::{handle_rejection, ErrorMessage},
    Server,
};
use rabbit_digger::controller::Controller;
use warp::{Filter, Rejection};

pub fn api(server: Server) -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
    let at = access_token(server.access_token);
    let prefix = warp::path!("api" / ..);

    prefix
        .and(at)
        .and(get_config(server.controller).or(not_found()))
        .recover(handle_rejection)
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

fn not_found() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path::end().map(|| {
        warp::reply::json(&ErrorMessage {
            code: 404,
            message: "Not found".into(),
        })
    })
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
