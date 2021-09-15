use std::{convert::Infallible, future, path::PathBuf};

use super::{handlers, reject::handle_rejection, reject::ApiError, Server};
use rabbit_digger::RabbitDigger;
use warp::{Filter, Rejection};

pub fn api(server: Server) -> impl Filter<Extract = impl warp::Reply, Error = Rejection> + Clone {
    let at = check_access_token(server.access_token);
    let prefix = warp::path!("api" / ..);
    // TODO: read or write userdata by API
    let userdata = &server
        .userdata
        .or_else(|| dirs::config_dir().map(|d| d.join("rabbit-digger")));
    let rd = &server.rabbit_digger;

    let get_config = warp::path!("config")
        .and(warp::get())
        .and(with_rd(rd))
        .and_then(handlers::get_config);
    let post_config = warp::path!("config")
        .and(warp::post())
        .and(with_rd(rd))
        .and(warp::body::json())
        .and_then(handlers::post_config);
    let get_registry = warp::path!("registry")
        .and(warp::get())
        .and(with_rd(rd))
        .and_then(handlers::get_registry);
    let get_connection = warp::path!("registry")
        .and(warp::get())
        .and(with_rd(rd))
        .and_then(handlers::get_connection);
    let get_state = warp::path!("state")
        .and(warp::get())
        .and(with_rd(rd))
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
    let delete_userdata = warp::path("userdata")
        .and(warp::delete())
        .and(with_userdata(userdata))
        .and(warp::path::tail())
        .and_then(handlers::delete_userdata);

    prefix.and(
        ws_event(&server.rabbit_digger)
            .or(at.and(
                get_config
                    .or(post_config)
                    .or(get_registry)
                    .or(get_connection)
                    .or(get_state)
                    .or(get_userdata)
                    .or(put_userdata)
                    .or(delete_userdata),
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
        .allow_methods(["GET", "POST", "PUT", "DELETE"]);

    api(server).or(forward).with(cors)
}

// Websocket /event
pub fn ws_event(
    rd: &RabbitDigger,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("event")
        .and(with_rd(rd))
        .and(warp::ws())
        .and_then(handlers::ws_event)
}

fn with_rd(
    rd: &RabbitDigger,
) -> impl Filter<Extract = (RabbitDigger,), Error = Infallible> + Clone {
    let rd = rd.clone();
    warp::any().map(move || rd.clone())
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
