use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use tracing::{info, warn};

use crate::{
    config::{AzureAKSConfig, AzureSDKConfig},
    Error, OAuthToken,
};

/// Trait to implement for types that capable of providing an access token that can be exchanged for a refresh token,
///
#[async_trait]
pub trait AccessProvider {
    /// Returns an access token that can be exchanged for a refresh token,
    ///
    async fn access_token(&self) -> Result<String, Error>;

    /// Returns a tenant id if relevant,
    ///
    fn tenant_id(&self) -> Option<String> {
        None
    }
}

/// Returns the default access provider,
///
pub fn default_access_provider(
    access_token_path: Option<PathBuf>,
) -> Arc<dyn AccessProvider + Send + Sync + 'static> {
    if let Some(aks_config) = AzureAKSConfig::try_load().ok() {
        info!("AKS config detected, using AKS as the access provider");
        Arc::new(aks_config)
    } else if let Some(path) = access_token_path {
        info!(
            "File access_token provided, using {:?} as the access provider",
            path
        );
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
            let token = OAuthToken::read_token_cache(self).await?;
            Ok(token.token())
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use std::{
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    use crate::AccessProvider;

    #[tokio::test]
    async fn test_pathbuf_accessprovider() {
        let expires_on = SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap()
            .checked_add(Duration::new(3000, 0))
            .unwrap()
            .as_secs();
        let test_file = format!(
            r#"
        {{
            "refresh_token": "test_token",
            "claims": {{
                "exp": {expires_on}
            }}
        }}
        "#
        );

        let test_file_path = PathBuf::from(".test/test_access_provider");

        std::fs::create_dir_all(".test").expect("should be able to create test dir");

        std::fs::write(&test_file_path, test_file).expect("should be able to write");

        let token = test_file_path.access_token().await.expect("should return a token");
        assert_eq!("test_token", token.as_str());
    }
}
