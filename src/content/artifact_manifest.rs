use std::collections::BTreeMap;

use crate::ProxyTarget;
use hyper::Method;
use lifec::prelude::{BlobSource, ContentBroker, MemoryBlobSource, Sha256Digester, ThunkContext, Component, DefaultVecStorage};
use tracing::{event, Level};
use std::io::Write;
use serde::{Deserialize, Serialize};

use super::Descriptor;

/// Manifest struct for stored artifacts related to an image
///
/// Artifacts are data related to the image, but that are not directly part of any of the
/// image layers that make up the container's filesystem.
///
/// An artifact can be literally anything, but example usages include sbom's, signatures, etc.
///
#[derive(Default, Component, Clone, Deserialize, Serialize, Debug)]
#[storage(DefaultVecStorage)]
pub struct ArtifactManifest {
    /// Media type, for this manifest it should be application/vnd.oci.artifact.manifest.v1+json
    ///
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// Artifact type name
    ///
    #[serde(rename = "artifactType")]
    pub artifact_type: String,
    /// List of descriptors for each blob in the manifest
    ///
    #[serde(rename = "blobs")]
    pub blobs: Vec<Descriptor>,
    /// Subject is the owner of this artifact,
    ///
    #[serde(rename = "subject")]
    pub subject: Descriptor,
    /// Optional, labels
    ///
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,
}

impl ArtifactManifest {
    /// Uploads this artifact to the proxy_target found in the thunk_context
    ///
    pub async fn upload(&self, thunk_context: &ThunkContext) {
        if let Some(proxy_target) = ProxyTarget::try_from(thunk_context).ok() {
            let request = proxy_target.start_request();
            let bytes = serde_json::to_vec(&self).expect("should be serializable");

            // TODO -- this will eventually be generalized
            let mut blob_source = MemoryBlobSource::default();
            blob_source
                .new("")
                .as_mut()
                .write_all(&bytes)
                .expect("can write");
            let digester = Sha256Digester().format(blob_source);
            let blob_device = digester
                .hash_map()
                .drain()
                .take(1)
                .next()
                .expect("should exist");

            let request = request
                .content_type(&self.media_type)
                .uri_str(proxy_target.manifest_with(blob_device.0))
                .method(Method::PUT)
                .body(bytes);

            let response = proxy_target
                .send_request(request)
                .await
                .expect("should get a response back");

            if response.status().is_success() {
                event!(
                    Level::DEBUG,
                    "Pushed manifest, Location: {:?}",
                    response.headers().get("Location")
                );
            } else {
                match hyper::body::to_bytes(response.into_body()).await {
                    Ok(data) => {
                        event!(Level::DEBUG, "Resolved blob, len: {}", data.len());
                        event!(Level::TRACE, "{:#?}", data);
                    }
                    Err(err) => event!(Level::ERROR, "{err}"),
                }
            }
        }
    }
}

pub mod consts {
    /// Media type for artifact manifests, documented here https://github.com/oras-project/artifacts-spec/blob/main/artifact-manifest.md
    ///
    pub const ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE: &'static str =
        "application/vnd.cncf.oras.artifact.manifest.v1+json";
    
    /// Media type for artifacts manifests, documented here https://github.com/opencontainers/image-spec/blob/main/artifact.md
    ///
    pub const OCI_ARTIFACTS_MANIFEST_MEDIA_TYPE: &'static str =
        "application/vnd.oci.artifact.manifest.v1+json";
}

