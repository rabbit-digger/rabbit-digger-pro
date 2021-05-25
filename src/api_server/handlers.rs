use std::convert::Infallible;

use rabbit_digger::controller::Controller;

pub async fn get_config(ctl: Controller) -> Result<impl warp::Reply, Infallible> {
    let ctl = ctl.lock().await;

    Ok(warp::reply::json(&ctl.config()))
}

pub async fn get_registry(ctl: Controller) -> Result<impl warp::Reply, Infallible> {
    let ctl = ctl.lock().await;

    Ok(warp::reply::json(&ctl.registry()))
}

pub async fn get_state(ctl: Controller) -> Result<impl warp::Reply, Infallible> {
    let ctl = ctl.lock().await;

    Ok(warp::reply::json(&ctl.state()))
}
