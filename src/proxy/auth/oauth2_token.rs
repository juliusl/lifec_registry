use hyper::{header::WWW_AUTHENTICATE, Uri};
use lifec::prelude::SecureClient;
use serde::{Deserialize, Serialize};
use crate::{BearerChallengeConfig, Error};

/// Struct to that contains an OAuth2 access_token,
///
#[derive(Serialize, Deserialize)]
pub struct OAuthToken {
    /// The remote host this token is intended for,
    #[serde(skip)]
    host: String,
    /// Access token that can be used to exchange for a new refresh_token
    /// 
    access_token: Option<String>,
    /// Refresh token that can be used to exchange for an access_token for resources
    /// 
    refresh_token: Option<String>,
}

impl OAuthToken {
    /// Returns the host this access_token is intended for,
    /// 
    pub fn host(&self) -> String {
        self.host.to_string()
    }

    /// Returns the token in context,
    /// 
    pub fn token(&self) -> String {
        if let Some(refresh_token) = self.refresh_token.as_ref() {
            refresh_token.to_string()
        } else if let Some(access_token) = self.access_token.as_ref() {
            access_token.to_string()
        } else {
            String::default()
        }
    }
    
    /// Authorizes a remote_uri, returns self if successful, otherwise returns an error,
    ///
    /// Authorizes w/ the current environment to get an up-to-date refresh_token,
    /// 
    pub async fn refresh_token(
        client: SecureClient,
        remote_uri: impl Into<String>,
        access_token: String,
        tenant_id: Option<String>
    ) -> Result<Self, Error> {
        let uri = remote_uri.into().parse::<Uri>()?;

        if let Some(challenge) = client.get(uri.clone()).await?.headers().get(WWW_AUTHENTICATE) {
            let oauth_config = BearerChallengeConfig::parse_from_header(challenge)?
                .exchange(access_token, tenant_id.unwrap_or(String::from("common")))
                .build_request()?;

            let mut response = client.request(oauth_config).await?;

            if !response.status().is_success() {
                return Err(Error::external_dependency_with(response.status()));
            }

            let bytes = hyper::body::to_bytes(response.body_mut()).await?;

            let mut token = serde_json::from_slice::<OAuthToken>(&bytes)?;
            if let Some(host) = uri.host().as_ref() {
                token.host = host.to_string();
            }

            Ok(token)
        } else {
            Err(Error::invalid_operation("The remote uri did not return a challenge header"))
        }
    }

    /// Authorizes a remote_uri, returns self if successful, otherwise returns an error,
    /// 
    /// Authorizes w/ the refresh token in order to get a new access_token
    /// 
    pub async fn access_token(
        client: SecureClient,
        remote_uri: impl Into<String>,
        refresh_token: String,
    ) -> Result<Self, Error> {
        let uri = remote_uri.into().parse::<Uri>()?;

        if let Some(challenge) = client.get(uri.clone()).await?.headers().get(WWW_AUTHENTICATE) {
            let oauth_config = BearerChallengeConfig::parse_from_header(challenge)?
                .token_by_refresh_token(refresh_token)
                .build_request()?;

            let mut response = client.request(oauth_config).await?;

            let bytes = hyper::body::to_bytes(response.body_mut()).await?;

            let mut token = serde_json::from_slice::<OAuthToken>(&bytes)?;
            if let Some(host) = uri.host().as_ref() {
                token.host = host.to_string();
            }

            Ok(token)
        } else {
            Err(Error::invalid_operation("The remote uri did not return a challenge header"))
        }
    }
}
