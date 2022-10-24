use std::collections::BTreeMap;

use lifec::prelude::{AttributeIndex, Component, DefaultVecStorage, ThunkContext};
use serde::{Deserialize, Serialize};

use super::Platform;

/// Registry descriptor data layout
///
/// A descriptor is a common specification registries use to reference content, this struct is a
/// combination of different descriptor layouts into a single layout.
///
/// Caveat: The content of a descriptor matters, once a client pushes a descriptor to a registry,
/// **no** fields may change, this will change the effective content digest.
///
#[derive(Default, Component, Clone, Deserialize, Serialize, Debug)]
#[storage(DefaultVecStorage)]
pub struct Descriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    #[serde(rename = "artifactType")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
    #[serde(rename = "digest")]
    pub digest: String,
    #[serde(rename = "size")]
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "annotations")]
    pub annotations: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "urls")]
    pub urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "data")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
}

impl Descriptor {
    /// Extracts a descriptor from a thunk context, 
    /// 
    pub fn extract(tc: &ThunkContext) -> Option<Self> {
        match (
            tc.search().find_symbol("content-type"),
            tc.search().find_binary("body"),
            tc.search().find_symbol("digest"),
            tc.search().find_symbol("artifact-type"),
        ) {
            (Some(media_type), Some(body), Some(digest), artifact_type) => Some(Descriptor {
                media_type,
                artifact_type,
                digest,
                size: body.len() as u64,
                annotations: None,
                urls: None,
                data: None,
                platform: None,
            }),
            _ => None,
        }
    }
}
