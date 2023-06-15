mod login_response;
use std::sync::Arc;

use hyper::Method;
pub use login_response::LoginResponse;

use poem::error::IntoResult;
use poem::handler;
use poem::web::Data;
use poem::web::Json;
use poem::IntoResponse;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::RwLock;
use tracing::debug;

use crate::config::LoginConfig;
use crate::Error;

#[derive(Serialize, Deserialize)]
pub struct LoginBody {
    pub(crate) host: String,
    pub(crate) username: String,
    pub(crate) password: String,
}

/// Handles managing registry logins,
/// 
#[handler]
pub async fn handle_login(
    method: Method,
    Json(LoginBody {
        host,
        username,
        password,
    }): Json<LoginBody>,
    login_config: Data<&Arc<RwLock<LoginConfig>>>,
) -> Result<LoginResponse, Error> {
    if method != Method::PUT {
        Err(Error::invalid_operation("Only PUT method is supportted"))?
    }

    debug!("Received login for {host}, waiting for write permissions");
    let overwritten = login_config.write().await.login(&host, username, password)?;

    debug!(overwritten, "Completed login for {host}");
    Ok(LoginResponse {
        overwritten,
        error: None,
    })
}

impl IntoResult<LoginResponse> for Result<LoginResponse, Error> {
    fn into_result(self) -> poem::Result<LoginResponse> {
        match self {
            Ok(resp) => Ok(resp),
            Err(err) => {
                let resp = LoginResponse::error(err);
                let resp = resp.into_response();
                Err(poem::Error::from_response(resp))
            }
        }
    }
}
