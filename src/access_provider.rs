use std::{sync::Arc, path::PathBuf};

use async_trait::async_trait;
use tracing::{info, warn};

use crate::{Error, config::{AzureSDKConfig, AzureAKSConfig}};

/// Trait to implement for types that capable of providing an access token that can be exchanged for a refresh token,
/// 
#[async_trait]
pub trait AccessProvider {
    /// Returns an access token that can be exchanged for a refresh token,
    /// 
    async fn access_token(&self) -> Result<String, Error>;
}

/// Returns the default access provider,
/// 
pub fn default_access_provider(access_token_path: Option<PathBuf>) -> Arc<dyn AccessProvider + Send + Sync + 'static> {
    if let Some(aks_config) = AzureAKSConfig::try_load().ok() {
        info!("AKS config detected, using AKS as the access provider");
        Arc::new(aks_config)
    } else if let Some(path) = access_token_path {
        info!("File access_token provided, using {:?} as the access provider", path);
        warn!("If this file is deleted the fallback will be the Azure SDK access provider");
        Arc::new(path)
    } else {
        info!("Azure SDK will be used as the access provider");
        Arc::new(AzureSDKConfig::default())
    }
}

#[async_trait]
impl AccessProvider for PathBuf {
    async fn access_token(&self) -> Result<String, Error> {
        if !self.exists() {
            warn!("{:?} has been deleted, falling back to Azure SDK", self);
            Ok(AzureSDKConfig::default().access_token().await?)
        } else {
            let access_token = tokio::fs::read_to_string(self).await?;

            Ok(access_token)
        }
    }
}