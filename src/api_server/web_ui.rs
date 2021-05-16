#![allow(unused_mut, dead_code)]

use std::{io, path::PathBuf};

use futures::{stream::BoxStream, StreamExt};
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, Framed};
use urlencoding::decode;
use warp::{hyper::body::Bytes, path::FullPath};

const HTTP_PREFIX: &'static str = "http://";

pub async fn web_ui(
    path: FullPath,
    web_ui: Option<String>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let web_ui = web_ui.ok_or(warp::reject::not_found())?;
    let mut builder = warp::http::Response::builder();

    #[cfg(feature = "http_web_ui")]
    let stream = if web_ui.starts_with(HTTP_PREFIX) {
        // HTTP web_ui
        use futures::TryStreamExt;

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
        file_stream(web_ui, path).await?
    };

    #[cfg(not(feature = "http_web_ui"))]
    let stream = file_stream(web_ui, path).await?;

    builder
        .body(warp::hyper::Body::wrap_stream(stream))
        .map_err(|_| warp::reject::not_found())
}

async fn file_stream(
    web_ui: String,
    path: FullPath,
) -> Result<BoxStream<'static, io::Result<Bytes>>, warp::Rejection> {
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

    Ok(stream)
}
