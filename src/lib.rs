
mod content;
pub use content::Platform;
pub use content::ReferrersList;
pub use content::Descriptor;
pub use content::ArtifactManifest;
pub use content::ImageIndex;
pub use content::ImageManifest;
pub use content::OCI_ARTIFACTS_MANIFEST_MEDIA_TYPE;
pub use content::ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE;

mod plugins;
pub use plugins::Mirror;
pub use plugins::Artifact;
pub use plugins::Authenticate;
pub use plugins::Continue;
pub use plugins::FormatOverlayBD;
pub use plugins::Login;
pub use plugins::LoginACR;
pub use plugins::LoginOverlayBD;
pub use plugins::Discover;
pub use plugins::Import;
pub use plugins::Teleport;
pub use plugins::Resolve;

mod proxy;
pub use proxy::RegistryProxy;
pub use proxy::ProxyTarget;

