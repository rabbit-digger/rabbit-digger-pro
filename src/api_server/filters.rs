use std::convert::Infallible;

use super::handlers;
use rabbit_digger::controller::Controller;
use warp::Filter;

pub fn api(
    ctl: Controller,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    get_config(ctl)
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
