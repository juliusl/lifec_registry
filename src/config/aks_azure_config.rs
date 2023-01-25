use serde::{Serialize, Deserialize};


/// This struct is the format of /etc/kubernetes/azure.json
/// 
#[derive(Serialize, Deserialize)]
pub struct AKSAzureConfig {
    /// Current cloud environment
    /// 
    cloud: String,
    /// AAD Tenant ID 
    /// 
    #[serde(rename = "tenantId", default = "String::default")]
    tenant_id: String,
    /// Service Principal Client ID
    /// 
    #[serde(rename = "aadClientId", default = "String::default")]
    aad_client_id: String,
    /// Service Principal Client Secret
    /// 
    #[serde(rename = "aadClientSecret", default = "String::default")]
    aad_client_secret: String,
    /// Service Principal Client Certificate
    /// 
    #[serde(rename = "aadClientCertPath", default = "String::default")]
    aad_client_cert_path: String,
    /// Service Principal Client Password
    /// 
    #[serde(rename = "aadClientCertPassword", default = "String::default")]
    aad_client_cert_password: String,
    /// If true, uses the managed identity token to fetch a service principal token
    /// 
    #[serde(rename = "useManagedIdentityExtension")]
    use_managed_identity_extension: bool,
    /// User Assigned identity client id
    /// 
    #[serde(rename = "userAssignedIdentityId", default = "String::default")]
    user_assigned_identity_id: String,
    /// Subscription id 
    /// 
    #[serde(rename = "subscriptionId", default = "String::default")]
    subscription_id: String,
    /// Identity System, possible values are azure_ad or adfs
    /// 
    #[serde(rename = "identitySystem", default = "String::default")]
    identity_system: String,
    /// Resource management endpoint
    /// 
    #[serde(rename = "resourceManagerEndpoint", default = "String::default")]
    resource_manager_endpoint: String,
    /// AAD Tenant ID for network resources
    /// 
    #[serde(rename = "networkResourceTenantID", default = "String::default")]
    network_resource_tenant_id: String,
    /// Subscription ID of network resources
    /// 
    #[serde(rename = "networkResourceSubscriptionID", default = "String::default")]
    network_resource_subscription_id: String
}