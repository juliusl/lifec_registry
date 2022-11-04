use std::collections::BTreeMap;

use serde::{Serialize, Deserialize};
use specs::{Component, VecStorage};

use super::Descriptor;

/// Struct for an image manifest,
/// 
#[derive(Component, Debug, Default, Clone, Serialize, Deserialize)]
#[storage(VecStorage)]
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

/// OCI image manifest media type,
/// 
pub const OCI_IMAGE_MANIFEST: &'static str  = "application/vnd.oci.image.manifest.v1+json";

/// Docker V1 manifest media type,
/// 
pub const DOCKER_V1_MANIFEST: &'static str  = "application/vnd.docker.distribution.manifest.v1+json";

/// Docker V2 manifest media type,
/// 
pub const DOCKER_V2_MANIFEST: &'static str  = "application/vnd.docker.distribution.manifest.v2+json";
