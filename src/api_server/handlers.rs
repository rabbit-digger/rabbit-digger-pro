use std::{convert::Infallible, io, path::PathBuf};

use futures::{StreamExt, TryStreamExt};
use rabbit_digger::controller::Controller;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, Framed};
use urlencoding::decode;
use warp::path::FullPath;

const HTTP_PREFIX: &'static str = "http://";

pub async fn get_config(ctl: Controller) -> Result<impl warp::Reply, Infallible> {
    let ctl = ctl.lock().await;
    let config = ctl.config();

    Ok(warp::reply::json(&config))
}

pub async fn web_ui(
    path: FullPath,
    web_ui: Option<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let web_ui = web_ui.ok_or(warp::reject::not_found())?;
    let mut builder = warp::http::Response::builder();

    let stream = if web_ui.starts_with(HTTP_PREFIX) {
        // HTTP web_ui
        let resp = reqwest::get(web_ui + path.as_str())
            .await
            .map_err(|_| warp::reject::not_found())?;

        builder = builder.status(resp.status());
        for (name, value) in resp.headers() {
            builder = builder.header(name, value);
        }

        let stream = warp::hyper::Body::wrap_stream(resp.bytes_stream())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .boxed();

        stream
    } else {
        // File web_uid
        let path = decode(&path.as_str()[1..]).map_err(|_| warp::reject::not_found())?;
        let root_path = PathBuf::from(web_ui);
        let file_path = root_path.join(PathBuf::from(path));
        let file = match File::open(file_path).await {
            Ok(f) => f,
            Err(_) => {
                let index = root_path.join("index.html");
                File::open(index)
                    .await
                    .map_err(|_| warp::reject::not_found())?
            }
        };
        let stream = Framed::new(file, BytesCodec::new())
            .map(|i| i.map(|i| i.freeze()))
            .boxed();

        stream
    };
    builder
        .body(warp::hyper::Body::wrap_stream(stream))
        .map_err(|_| warp::reject::not_found())
}
