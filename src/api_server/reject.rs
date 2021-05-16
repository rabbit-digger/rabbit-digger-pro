use std::convert::Infallible;

use serde_derive::Serialize;
use warp::{hyper::StatusCode, Rejection, Reply};

#[derive(Debug)]
pub enum ApiError {
    Forbidden,
}
impl warp::reject::Reject for ApiError {}

/// An API error serializable to JSON.
#[derive(Serialize)]
pub struct ErrorMessage {
    pub code: u16,
    pub message: String,
}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "NOT_FOUND";
    } else if let Some(ApiError::Forbidden) = err.find() {
        code = StatusCode::FORBIDDEN;
        message = "Forbidden"
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
