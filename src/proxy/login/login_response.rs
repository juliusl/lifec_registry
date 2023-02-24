use hyper::StatusCode;
use poem::IntoResponse;
use serde::Serialize;

use crate::Error;

/// Struct containing the response for a login request,
///
#[derive(Serialize)]
pub struct LoginResponse {
    /// True if existing login credentials were overwritten,
    ///
    pub(crate) overwritten: bool,
    /// Error for this response,
    ///
    #[serde(skip)]
    pub(crate) error: Option<Error>,
}

impl LoginResponse {
    /// Returns an error response,
    ///
    pub fn error(err: Error) -> Self {
        Self {
            overwritten: false,
            error: Some(err),
        }
    }
}

impl IntoResponse for LoginResponse {
    fn into_response(self) -> poem::Response {
        let response = poem::Response::builder().status(if self.error.is_none() {
            StatusCode::OK
        } else if self
            .error
            .as_ref()
            .map(|e| e.is_invalid_operation())
            .unwrap_or_default()
        {
            StatusCode::METHOD_NOT_ALLOWED
        } else if self
            .error
            .as_ref()
            .map(|e| e.is_recoverable())
            .unwrap_or_default()
        {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        });

        if let Some(error) = self.error.as_ref() {
            response.body(format!("{error}"))
        } else {
            response.finish()
        }
    }
}
