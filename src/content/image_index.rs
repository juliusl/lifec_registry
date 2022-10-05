use serde::{Deserialize, Serialize};

use crate::Descriptor;

/// Struct for an image manifest,
/// 
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImageIndex {
    /// Schema version of this manifest
    /// 
    #[serde(rename = "schemaVersion")]
    pub schema_versin: usize,
    /// Media type, for this manifest it should be application/vnd.oci.artifact.manifest.v1+json
    /// 
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// List of manifests contained within this index
    /// 
    pub manifests: Vec<Descriptor> 
}