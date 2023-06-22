use std::sync::Arc;

use hyper::StatusCode;
use lifec::prelude::ThunkContext;
use poem::{
    handler,
    web::{Data, Query}, error::IntoResult,
};
use serde::{Deserialize, Serialize};

mod auth_response;
use auth_response::AuthResponse;

mod oauth2_token;
pub use oauth2_token::OAuthToken;
use tokio::sync::RwLock;
use tracing::{error, info};
use url::Url;

use crate::{Error, AccessProvider, config::LoginConfig};

/// Struct for a request to authenticate a registry request,
///
#[derive(Serialize, Deserialize)]
pub struct AuthRequest {
    remote_url: String,
}

#[handler]
pub async fn handle_auth(
    Query(AuthRequest { remote_url }): Query<AuthRequest>,
    context: Data<&ThunkContext>,
    access_provider: Data<&Arc<dyn AccessProvider + Send + Sync + 'static>>,
    login_config: Data<&Arc<RwLock<LoginConfig>>>,
) -> Result<AuthResponse, Error> {
    let url: Url = remote_url.parse()?;
    if let Some(domain) = url.domain() {
        if let Some((username, password)) = login_config.read().await.authorize(domain) {
            info!("Login credentials found for {domain}, using those instead of token access");
            return Ok(AuthResponse::login(domain, username, password));
        }
    }

    info!("Request to authenticate {remote_url}");
    let access_token = access_provider.access_token().await?;
    let client = context.client().expect("should have an https client");

    let refresh_token = OAuthToken::exchange_token(
        client, 
        remote_url, 
        access_token, 
        access_provider.tenant_id()
    ).await?;

    Ok(AuthResponse::authorize(refresh_token.host(), refresh_token.token()))
}

impl IntoResult<AuthResponse> for Result<AuthResponse, Error> {
    fn into_result(self) -> poem::Result<AuthResponse> {
        match self {
            Ok(resp) => {
                Ok(resp)
            },
            Err(err) => {
                error!("Server ran into an error {err}");
                Err(poem::Error::from_string(err.to_string(), StatusCode::SERVICE_UNAVAILABLE))
            },
        }
    }
}