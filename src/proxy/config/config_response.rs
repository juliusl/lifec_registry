use hyper::StatusCode;
use poem::IntoResponse;

use crate::Error;

/// Struct to return in response to /config
/// 
#[derive(Debug)]
pub struct ConfigResponse {
    /// True if the config was installed,
    /// 
    installed: bool,
    /// Error
    /// 
    error: Option<Error>,
}

impl ConfigResponse {
    /// Creates a new ok response,
    /// 
    pub fn ok() -> Self {
        ConfigResponse { installed: true, error: None }
    }

    /// Creates a new ok uninstalled response,
    /// 
    pub fn ok_uninstalled() -> Self {
        ConfigResponse { installed: false, error: None }
    }

    /// Creates a new error response
    /// 
    pub fn error(error: Error) -> Self {
        ConfigResponse { installed: false, error: Some(error) }
    }
}

impl IntoResponse for ConfigResponse {
    fn into_response(self) -> poem::Response {
        let response = poem::Response::builder()
            .status(if self.installed && self.error.is_none() {
                StatusCode::OK
            } else if self.error.as_ref().map(|e| e.is_invalid_operation()).unwrap_or_default() {
                StatusCode::METHOD_NOT_ALLOWED
            } else if self.error.as_ref().map(|e| e.is_recoverable()).unwrap_or_default() {
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