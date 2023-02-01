use async_trait::async_trait;
use azure_core::auth::TokenCredential;
use azure_identity::DefaultAzureCredential;
use lifec::prelude::SecureClient;

use crate::{AccessProvider, Error};


/// Pointer struct to implement an AccessProvider for the Azure SDK
/// 
#[derive(Default)]
pub struct AzureSDKConfig; 

#[async_trait]
impl AccessProvider for AzureSDKConfig {
    async fn access_token(&self) -> Result<String, Error> {
        let creds = DefaultAzureCredential::default();
        let token = creds.get_token("https://management.azure.com/").await?;

        Ok(token.token.secret().to_string())
    }
}