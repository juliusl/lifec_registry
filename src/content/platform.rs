use serde::{Serialize, Deserialize};

/// Platform field of an image descriptor
/// 
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub architecture: String,
    pub os: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<String>,
}