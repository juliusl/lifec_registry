use lifec::{AttributeIndex, ThunkContext};

use crate::{ArtifactManifest, Descriptor, ImageIndex, ImageManifest};

/// Enumeration of possible manifest types,
///
#[derive(Debug, Clone)]
pub enum Manifests {
    Image(Descriptor, ImageManifest),
    Artifact(Descriptor, ArtifactManifest),
    Index(Descriptor, ImageIndex),
}

impl Manifests {
    /// Copies manifest to context for later processing,
    /// 
    pub fn copy_to_context(&self, context: &mut ThunkContext) {
        match &self {
            Manifests::Image(desc, manifest) => {
                if let Some(bytes) = serde_json::to_vec_pretty(manifest).ok() {
                    context
                        .state_mut()
                        .with_symbol("manifest", &desc.media_type)
                        .with_binary(&desc.media_type, bytes.to_vec())
                        .with_symbol("content-type", &desc.media_type)
                        .with_symbol("digest", &desc.digest);
                }
            }
            Manifests::Artifact(desc, manifest) => {
                if let Some(bytes) = serde_json::to_vec_pretty(manifest).ok() {
                    context
                        .state_mut()
                        .with_symbol("manifest", &desc.media_type)
                        .with_binary(&desc.media_type, bytes.to_vec())
                        .with_symbol(
                            "artifact-type",
                            &desc
                                .artifact_type
                                .as_ref()
                                .expect("should have an artifact type"),
                        )
                        .with_symbol("content-type", &desc.media_type)
                        .with_symbol("digest", &desc.digest);
                }
            }
            Manifests::Index(desc, manifest) => {
                if let Some(bytes) = serde_json::to_vec_pretty(manifest).ok() {
                    context
                        .state_mut()
                        .with_symbol("manifest", &desc.media_type)
                        .with_binary(&desc.media_type, bytes.to_vec())
                        .with_symbol("content-type", &desc.media_type)
                        .with_symbol("digest", &desc.digest);
                }
            }
        }
    }
}
