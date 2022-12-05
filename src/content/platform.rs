use serde::{Serialize, Deserialize};
use specs::{Component, VecStorage};

/// Platform field of an image descriptor
/// 
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
#[storage(VecStorage)]
pub struct Platform {
    /// Architecture
    pub architecture: String,
    /// Operating system
    pub os: String,
    /// Operating system variant
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<String>,
}