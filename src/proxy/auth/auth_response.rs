use std::collections::BTreeMap;

use hyper::{Body, Response, StatusCode};
use poem::IntoResponse;
use serde::Serialize;

/// Struct to serialize an auth response,
///
#[derive(Serialize, Default)]
pub struct AuthResponse {
    #[serde(rename = "traceId")]
    trace_id: String,
    success: bool,
    data: Option<AuthData>,
}

/// Struct to serialize auth data,
///
#[derive(Serialize, Default)]
struct AuthData {
    auths: BTreeMap<String, AuthCreds>,
}

/// Struct to serialize auth credentials,
///
#[derive(Serialize, Default)]
pub struct AuthCreds {
    pub username: String,
    pub password: String,
}

impl AuthResponse {
    /// Returns a failed auth response,
    ///
    pub fn unauthorized() -> Response<Body> {
        AuthResponse {
            trace_id: "${trace_id}".to_string(),
            success: false,
            data: None,
        }.create_response(StatusCode::UNAUTHORIZED)
    }


    /// Returns an authorized response,
    /// 
    pub fn authorize(host: String, refresh_token: String) -> AuthResponse {
        let creds = AuthCreds {
            username: "00000000-0000-0000-0000-000000000000".to_string(),
            password: refresh_token,
        };

        let mut auth_data = AuthData::default();
        auth_data.auths.insert(host, creds);

        AuthResponse {
            trace_id: "${trace_id}".to_string(),
            success: true,
            data: Some(auth_data),
        }
    }

    /// Returns a response w/ login credentials,
    /// 
    pub fn login(host: impl Into<String>, username: impl Into<String>, password: impl Into<String>) -> AuthResponse {
        let mut auth_data = AuthData::default();
        auth_data.auths.insert(host.into(), AuthCreds { username: username.into(), password: password.into() });

        AuthResponse {
            trace_id: "${trace_id}".to_string(),
            success: true,
            data: Some(auth_data),
        }
    }

    /// Returns a response from current state,
    /// 
    pub fn create_response(&self, status_code: StatusCode) -> Response<Body> {
        let auth_response =
            serde_json::to_vec(self).expect("should be able to serialize this");

        Response::builder()
            .status(status_code)
            .body(Body::from(auth_response))
            .expect("should always be able to create this response")
    }
}

impl IntoResponse for AuthResponse {
    fn into_response(self) -> poem::Response {
        let auth_response =
            serde_json::to_vec(&self).expect("should be able to serialize this");

        let status_code = if self.success {
            StatusCode::OK
        } else {
            StatusCode::UNAUTHORIZED
        };

        poem::Response::builder().status(status_code).body(Body::from(auth_response))
    }
}