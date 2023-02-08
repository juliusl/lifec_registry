#[doc(hidden)]
#[macro_use]
pub mod macros;

mod content;
pub use content::Platform;
pub use content::ReferrersList;
pub use content::Descriptor;
pub use content::ArtifactManifest;
pub use content::ImageIndex;
pub use content::ImageManifest;
pub use content::Registry;
pub use content::consts;

mod plugins;
pub use plugins::Mirror;
pub use plugins::Artifact;
pub use plugins::Authenticate;
pub use plugins::Login;
pub use plugins::Discover;
pub use plugins::Teleport;
pub use plugins::Resolve;

cfg_editor! {
    pub use plugins::RemoteRegistry;
}

pub mod hosts_config {
    pub use crate::plugins::MirrorHost;
    pub use crate::plugins::DefaultHost;
}

mod proxy;
pub use proxy::RegistryProxy;
pub use proxy::ProxyTarget;
pub use proxy::Object;
pub use proxy::Manifests;
pub use proxy::Blobs;
pub use proxy::OAuthToken;

mod config;
pub use config::Host as RegistryHost;
pub use config::HostsConfig;
pub use config::OAuthConfig;
pub use config::BearerChallengeConfig;
pub use config::ContainerdConfig;

pub mod azure {
    pub use crate::config::AzureAKSConfig;
    pub use crate::config::AzureIMDSConfig;
}

mod access_provider;
pub use access_provider::AccessProvider;
pub use access_provider::default_access_provider;

mod error;
pub use error::Error;