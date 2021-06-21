use serde::Serialize;
use warp::{hyper::StatusCode, Rejection, Reply};

#[derive(Debug)]
pub enum ApiError {
    Forbidden,
    NotFound,
}
impl warp::reject::Reject for ApiError {}

#[derive(Debug)]
struct CustomReject(anyhow::Error);

impl warp::reject::Reject for CustomReject {}

pub(crate) fn custom_reject(error: impl Into<anyhow::Error>) -> warp::Rejection {
    warp::reject::custom(CustomReject(error.into()))
}

/// An API error serializable to JSON.
#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, warp::Rejection> {
    let code;
    let message;

    tracing::error!("handle_rejection: {:?}", err);

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "NOT_FOUND";
    } else if let Some(ApiError::Forbidden) = err.find() {
        code = StatusCode::FORBIDDEN;
        message = "Forbidden"
    } else if let Some(ApiError::NotFound) = err.find() {
        code = StatusCode::NOT_FOUND;
        message = "Not found"
    } else {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "Internal server error"
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message: message.into(),
    });

    Ok(warp::reply::with_status(json, code))
}
