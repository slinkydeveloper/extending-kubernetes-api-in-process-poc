//! Error handling in [`kube-rs-async`][crate]

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Possible errors when working with [`kube-rs-async`][crate]
#[derive(Error, Debug)]
pub enum Error {
    /// ApiError for when things fail
    ///
    /// This can be parsed into as an error handling fallback.
    /// Replacement data for reqwest::Response::error_for_status,
    /// which is often lacking in good permission errors.
    /// It's also used in `WatchEvent` from watch calls.
    ///
    /// It's quite common to get a `410 Gone` when the resourceVersion is too old.
    #[error("ApiError: {0} ({0:?})")]
    Api(#[source] ErrorResponse),

    /// Http based error
    #[error("HttpError: {0}")]
    HttpError(#[from] http::Error),

    /// Url conversion error
    #[error("InternalUrlError: {0}")]
    InternalUrlError(#[from] url::ParseError),

    /// Common error case when requesting parsing into own structs
    #[error("Error deserializing response")]
    SerdeError(#[from] serde_json::Error),

    /// Error building a request
    #[error("Error building request")]
    RequestBuild,

    /// Error sending a request
    #[error("Error executing request")]
    RequestSend,

    /// Error parsing a response
    #[error("Error parsing response")]
    RequestParse,

    /// An invalid method was used
    #[error("Invalid API method {0}")]
    InvalidMethod(String),

    /// A request validation failed
    #[error("Request validation failed with {0}")]
    RequestValidation(String),
}

/// An Error response from the API
#[derive(Error, Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
#[error("{message}: {reason}")]
pub struct ErrorResponse {
    /// The status
    pub status: String,
    /// A message about the error
    #[serde(default)]
    pub message: String,
    /// The reason for the error
    #[serde(default)]
    pub reason: String,
    /// The error code
    pub code: u16,
}
