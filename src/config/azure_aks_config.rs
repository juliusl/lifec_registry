use std::{path::PathBuf, fs::File};

use async_trait::async_trait;
use azure_core::auth::TokenCredential;
use azure_identity::{authority_hosts, TokenCredentialOptions};
use std::io::Read;
use serde::{Deserialize, Serialize};

use crate::{AccessProvider, Error};

use super::AzureIMDSConfig;

const AKSCONFIG_PATH: &'static str = "/etc/kubernetes/azure.json";

/// This struct is the format of /etc/kubernetes/azure.json,
///
/// The format of this file can be found here, https://github.com/Azure/AgentBaker/blob/9ff376555bfc58b910e97fb1717ccbfb8e3da975/parts/linux/cloud-init/artifacts/cse_config.sh#L113-L168
///
#[derive(Serialize, Deserialize)]
pub struct AzureAKSConfig {
    /// Current cloud environment
    ///
    cloud: String,
    /// AAD Tenant ID
    ///
    #[serde(rename = "tenantId", default = "String::default")]
    tenant_id: String,
    /// Subscription id
    ///
    #[serde(rename = "subscriptionId", default = "String::default")]
    subscription_id: String,
    /// Service Principal Client ID
    ///
    #[serde(rename = "aadClientId", default = "String::default")]
    aad_client_id: String,
    /// Service Principal Client Secret
    ///
    #[serde(rename = "aadClientSecret", default = "String::default")]
    aad_client_secret: String,
    /// Resource group name
    ///
    #[serde(rename = "resourceGroup", default = "String::default")]
    resource_group: String,
    /// Location name
    ///
    #[serde(rename = "location", default = "String::default")]
    location: String,
    /// VM Type
    ///
    #[serde(rename = "vmType", default = "String::default")]
    vm_type: String,
    /// Network subnet name
    ///
    #[serde(rename = "subnetName", default = "String::default")]
    subnet_name: String,
    /// Network security group name
    ///
    #[serde(rename = "securityGroupName", default = "String::default")]
    security_group_name: String,
    /// VNET name
    ///
    #[serde(rename = "vnetName", default = "String::default")]
    vnet_name: String,
    /// VNET Resource group name
    ///
    #[serde(rename = "vnetResourceGroup")]
    vnet_resource_group: String,
    /// If true, uses the managed identity token to fetch a service principal token
    ///
    #[serde(rename = "useManagedIdentityExtension")]
    use_managed_identity_extension: bool,
    /// User Assigned identity client id
    ///
    #[serde(rename = "userAssignedIdentityID", default = "String::default")]
    user_assigned_identity_id: String,
}

impl AzureAKSConfig {
    /// Loads data from the config file is present, otherwise returns None if the file does not exist
    /// 
    pub fn try_load() -> Result<Self, Error> {
        if PathBuf::from(AKSCONFIG_PATH).exists() {
            let file = std::fs::read_to_string(AKSCONFIG_PATH)?;

            let config = serde_json::from_str(file.as_str())?;

            Ok(config)
        } else {
            Err(Error::recoverable_error("AKS config file does not exist, will try an alternative access provider"))
        }
    }
}

#[async_trait]
impl AccessProvider for AzureAKSConfig {
    /// Returns an access token based on the settings of /etc/kubernetes/azure.json
    ///
    /// Tries several ways to provide an access token, if configured w/
    ///
    async fn access_token(&self) -> Result<String, Error> {
        if self.use_managed_identity_extension {
            // This means that we need to use the IMDS which is over http
            AzureIMDSConfig::new()
                .client_id(self.user_assigned_identity_id.to_string())
                .access_token()
                .await
        } else if !self.aad_client_id.is_empty() && !self.aad_client_secret.is_empty() {
            let client = azure_core::new_http_client();

            let options = match self.cloud.as_str() {
                "AzureChinaCloud" => {
                    TokenCredentialOptions::new(authority_hosts::AZURE_CHINA.to_string())
                }
                "AzureGermanCloud" => {
                    TokenCredentialOptions::new(authority_hosts::AZURE_GERMANY.to_string())
                }
                "AzureUSGovernment" => {
                    TokenCredentialOptions::new(authority_hosts::AZURE_GOVERNMENT.to_string())
                }
                _ => TokenCredentialOptions::default(),
            };

            let creds = azure_identity::ClientSecretCredential::new(
                client,
                self.tenant_id.to_string(),
                self.aad_client_id.to_string(),
                self.aad_client_secret.to_string(),
                options,
            );

            let token = creds.get_token("https://management.azure.com/").await?;

            Ok(token.token.secret().to_string())
        } else {
            Err(Error::invalid_operation(
                "AKS config does not have enough information to create an access token",
            ))
        }
    }
}
