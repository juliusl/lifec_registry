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
    /// Media type, for this manifest ;ost
    /// 
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// List of manifests contained within this index
    /// 
    pub manifests: Vec<Descriptor> 
}

/// Docker manifest list media type,
/// 
pub const DOCKER_MANIFEST_LIST: &'static str = "application/vnd.docker.distribution.manifest.list.v2+json";