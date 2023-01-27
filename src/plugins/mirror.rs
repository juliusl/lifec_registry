use crate::RegistryProxy;
use lifec::prelude::{
    AttributeIndex, AttributeParser, BlockObject, BlockProperties, Component, CustomAttribute,
    HashMapStorage, Plugin, ThunkContext,
};

use lifec_poem::AppHost;
use tracing::{event, Level};

mod default_host;
pub use default_host::DefaultHost;

mod mirror_host;
pub use mirror_host::MirrorHost;

/// Designed to be used w/ containerd's registry config described here:
/// https://github.com/containerd/containerd/blob/main/docs/hosts.md
///
/// To enable this feature, it consists of writing a hosts.toml under /etc/containerd/certs.d/{host_name}
///
/// Here is an example to run a simple test w/ this mirror:
/// ```toml
/// server = "https://registry-1.docker.io"
///
/// [host."http://localhost:5000"]
/// capabilities = [ "resolve", "pull" ]
/// skip_verify = true
/// ```
///
/// And, then to test, you can use ctr:
/// ```sh
/// sudo ctr images pull --hosts-dir "/etc/containerd/certs.d" docker.io/library/python:latest  
/// ```
///
/// To setup the runtime, you can enable this setting in /etc/containerd/config.toml
///
/// ```toml
/// config_path = "/etc/containerd/certs.d"
/// ```
///
#[derive(Component, Clone, Default)]
#[storage(HashMapStorage)]
pub struct Mirror;

impl Plugin for Mirror {
    fn symbol() -> &'static str {
        "mirror"
    }

    fn description() -> &'static str {
        "Hosts a registry mirror, to extend registry capabilities at runtime"
    }

    fn call(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.task(|cancel| {
            let mut tc = context.clone();
            async move {
                let app_host = tc
                    .search()
                    .find_symbol("app_host")
                    .expect("should have an app host");

                lifec::plugins::await_plugin::<AppHost<RegistryProxy>>(
                    cancel,
                    tc.with_symbol("app_host", app_host),
                    |tc| {
                        event!(Level::INFO, "Exiting");
                        Some(tc)
                    },
                )
                .await
            }
        })
    }

    /// This will add some custom attributes to the parser for handling environment setup,
    ///
    /// # Usage Example
    ///
    /// ```runmd
    /// ``` test containerd
    /// + .runtime
    /// : .mirror   azurecr.io
    /// : .server   https://example.azurecr.io
    /// : .host     localhost:5000, pull, resolve, push
    /// : .https    hosts.crt
    /// ```
    ///
    fn compile(parser: &mut AttributeParser) {
        if let Some(mut docs) = Self::start_docs(parser) {
            docs.as_mut().with_custom::<RegistryProxy>();
        }
    }
}

impl BlockObject for Mirror {
    fn query(&self) -> BlockProperties {
        BlockProperties::default().require("mirror")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
