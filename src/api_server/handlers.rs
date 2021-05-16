use std::convert::Infallible;

use rabbit_digger::controller::Controller;

pub async fn get_config(ctl: Controller) -> Result<impl warp::Reply, Infallible> {
    let ctl = ctl.lock().await;
    let config = ctl.config();

    Ok(warp::reply::json(&config))
}
