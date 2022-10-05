
use tracing::{event, Level};

use super::{ArtifactManifest, Descriptor};

/// Wrapper struct for the artifact manifest to upload when linking overlaybd images,
///
pub struct OverlaybdArtifact(ArtifactManifest);

impl OverlaybdArtifact {
    /// Creates a new overlaybd artifact to upload,
    ///
    pub fn new(subject: Descriptor, converted: Descriptor) -> OverlaybdArtifact {
        let mut artifact_manifest = ArtifactManifest::default();

        artifact_manifest.subject = subject;
        artifact_manifest.blobs.push(converted);
        artifact_manifest.artifact_type = "dadi.image.v1".to_string();
        artifact_manifest.media_type = crate::ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE.to_string();

        event!(Level::TRACE, "{:#?}", artifact_manifest);

        Self(artifact_manifest)
    }

    /// Returns the artifact manifest for this artifact,
    /// 
    pub fn artifact(&self) -> ArtifactManifest {
        self.0.clone()
    }
}
