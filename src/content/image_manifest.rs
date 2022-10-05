use std::collections::BTreeMap;

use serde::{Serialize, Deserialize};

use super::Descriptor;

/// Struct for an image manifest,
/// 
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImageManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_versin: usize,
    /// Media type, for this manifest it should be application/vnd.oci.artifact.manifest.v1+json
    /// 
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// Descriptor pointing to the config for this image,
    /// 
    pub config: Descriptor, 
    /// List of descriptors for each layer in the image
    /// 
    pub layers: Vec<Descriptor>,
    /// Indicates a relationship to the descriptor,
    /// 
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Descriptor>,
    /// Optional, labels
    /// 
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>> 
}