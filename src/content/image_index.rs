use serde::{Deserialize, Serialize};
use specs::{Component, VecStorage};

use crate::Descriptor;

/// Struct for an image manifest,
/// 
#[derive(Component, Debug, Default, Clone, Serialize, Deserialize)]
#[storage(VecStorage)]
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