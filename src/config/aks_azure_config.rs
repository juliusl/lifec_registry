use serde::{Serialize, Deserialize};


/// This struct is the format of /etc/kubernetes/azure.json,
/// 
/// The format of this file can be found here, https://github.com/Azure/AgentBaker/blob/9ff376555bfc58b910e97fb1717ccbfb8e3da975/parts/linux/cloud-init/artifacts/cse_config.sh#L113-L168
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