use async_trait::async_trait;
use hyper::{Request, Uri, Body};
use serde::Deserialize;

use crate::{AccessProvider, Error};

const DEFAULT_IMDS_TOKEN_ENDPONIT: &'static str = "http://169.254.169.254/metadata/identity/oauth2/token";

const DEFAULT_IMDS_API_VERSION: &'static str = "2018-02-01";

/// Enumeration of possible IMDS configurations
///
pub struct AzureIMDSConfig {
    /// The token endpoint to use,
    /// 
    token_endpoint: AzureIMDSEndpoint,
    client_id: Option<String>,
    resource: Option<String>,
    api_version: Option<String>,
}

pub enum AzureIMDSEndpoint {
    /// In this case the endpoint is the value of MSI_ENDPOINT
    ///
    Environment(String),
    /// The default endpoint is http://169.254.169.254/metadata/identity/oauth2/token
    ///
    Default,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct IMDSTokenResponse {
    access_token: String,
    expires_in: String,
    expires_on: String,
    not_before: String,
    resource: String,
    token_type: String,
}

impl AzureIMDSConfig {
    /// Returns a new azure imds config,
    ///
    pub fn new() -> Self {
        let endpoint = if let Some(endpoint) = std::env::var("MSI_ENDPOINT").ok() {
            AzureIMDSEndpoint::Environment(endpoint)
        } else {
            AzureIMDSEndpoint::Default
        };

        Self {
            token_endpoint: endpoint,
            client_id: None,
            resource: None,
            api_version: None,
        }
    }

    /// Sets the client id on the config,
    /// 
    pub fn client_id(mut self, client_id: String) -> Self {
        if !client_id.is_empty() {
            self.client_id = Some(client_id);
        }
        self
    }

    /// Sets the resource on the config, 
    /// 
    /// If not set the default value used will be "https://management.azure.com/"
    /// 
    pub fn resource(mut self, resource: String) -> Self {
        self.resource = Some(resource);
        self
    }

    /// Returns the token uri for fetching an access_token for the current managed identity, 
    /// 
    pub fn token_uri(&self) -> Result<Uri, Error> {
        let endpoint = match &self.token_endpoint {
            AzureIMDSEndpoint::Environment(endpoint) => {
                endpoint.to_string()
            }
            AzureIMDSEndpoint::Default => DEFAULT_IMDS_TOKEN_ENDPONIT.to_string(),
        };

        let api_version = self.api_version.clone().unwrap_or(DEFAULT_IMDS_API_VERSION.to_string());

        let resource = self.resource.clone().unwrap_or("https://management.azure.com/".to_string());

        let mut uri = format!("{endpoint}?api-version={api_version}&resource={resource}");

        if let Some(client_id) = self.client_id.as_ref() {
            uri = format!("{uri}&client_id={client_id}");
        }

        Ok(uri.parse()?)
    }
}

#[async_trait]
impl AccessProvider for AzureIMDSConfig {
    async fn access_token(&self) -> Result<String, Error> {
        let client = hyper::Client::new();

        let uri = self.token_uri()?;

        let request = Request::builder()
            .header("Metadata", "true")
            .uri(uri)
            .body(Body::empty())?;

        let mut response = client.request(request).await?;

        let body = hyper::body::to_bytes(response.body_mut()).await?;

        let response = serde_json::from_slice::<IMDSTokenResponse>(&body)?;

        Ok(response.access_token)
    }
}
