use std::collections::BTreeMap;

use lifec::prelude::{AttributeIndex, Component, DefaultVecStorage, ThunkContext};
use serde::{Deserialize, Serialize};
use tracing::trace;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "artifactType")]
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

    /// Returns the a stremable descriptor if the annotations are present in the current descriptor,
    ///
    /// Example artifact manifest, the descriptor will have the below annoations,
    /// ``` json
    /// {
    ///   "mediaType": "application/vnd.oci.artifact.manifest.v1+json",
    ///   "artifactType": "application/vnd.azure.artifact.streaming.link.v1",
    ///   "subject": {
    ///     "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
    ///     "digest": "sha256:820582b05253c2b968442b8af31d791ae64478bcc18e04826c5ce42f974d3272",
    ///     "size": 1574
    ///   },
    ///   "annotations": {
    ///     "streaming.mediaType": "application/vnd.docker.distribution.manifest.v2+json",
    ///     "streaming.digest": "sha256:7a04484f0ab4dcdcca8ed5b2f4ae74b06afc80bab39c143783307cfa459516db",
    ///     "streaming.size": "3356",
    ///     "streaming.format": "overlaydb",
    ///     "streaming.version": "v1",
    ///     "streaming.platform.os": "linux",
    ///      "streaming.platform.arch": "amd64"
    ///    }
    /// }
    /// ```
    ///
    pub fn try_parse_streamable_descriptor(&self) -> Option<Self> {
        if let Some(annotations) = self
            .annotations
            .as_ref()
            .and_then(|a| serde_json::to_string(a).ok())
        {
            match serde_json::from_str::<StreamingDescriptor>(annotations.as_str()) {
                Ok(streaming_desc) => Some(Descriptor {
                    media_type: streaming_desc.media_type,
                    artifact_type: None,
                    digest: streaming_desc.digest,
                    size: streaming_desc.size.parse().unwrap_or_default(),
                    annotations: None,
                    urls: None,
                    data: None,
                    platform: None,
                }),
                Err(err) => {
                    trace!(
                        "Could not find streaming descriptor from current descriptor, {:?}",
                        err
                    );
                    None
                }
            }
        } else {
            None
        }
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct StreamingDescriptor {
    #[serde(rename = "streaming.mediaType")]
    media_type: String,
    #[serde(rename = "streaming.digest")]
    digest: String,
    #[serde(rename = "streaming.size")]
    size: String,
    #[serde(rename = "streaming.format")]
    format: String,
    #[serde(rename = "streaming.version")]
    version: Option<String>,
    #[serde(rename = "streaming.platform.os")]
    platform_os: Option<String>,
    #[serde(rename = "streaming.platform.arch")]
    platform_arch: Option<String>,
}

#[allow(unused_imports)]
mod tests {
    use serde_json::json;

    use crate::Descriptor;

    #[test]
    #[tracing_test::traced_test]
    fn test() {
        let json = json!(
        {
            "mediaType": "",
            "digest": "",
            "size": 0,
            "annotations": {
                "streaming.mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                "streaming.digest": "sha256:7a04484f0ab4dcdcca8ed5b2f4ae74b06afc80bab39c143783307cfa459516db",
                "streaming.size": "3356",
                "streaming.format": "cimfs",
                "streaming.version": "v1",
                "streaming.platform.os": "windows",
                "streaming.platform.arch": "amd64"
              }
        });

        let descriptor =
            serde_json::from_value::<Descriptor>(json).expect("should be able to deserialize");

        let streaming_desc = descriptor
            .try_parse_streamable_descriptor()
            .expect("should be able to return streaming descriptor");

        assert_eq!(
            "application/vnd.docker.distribution.manifest.v2+json",
            streaming_desc.media_type
        );
        assert_eq!(
            "sha256:7a04484f0ab4dcdcca8ed5b2f4ae74b06afc80bab39c143783307cfa459516db",
            streaming_desc.digest
        );
        assert_eq!(3356, streaming_desc.size);
    }
}
