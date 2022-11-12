use lifec::{
    plugins,
    prelude::{BlockObject, BlockProperties, Plugin, ThunkContext},
    resources::Resources,
    state::AttributeIndex,
};
use rust_embed::RustEmbed;
use tokio::{select, sync::oneshot};
use tracing::{event, Level};

use crate::{
    plugins::guest::{AzureDispatcher, AzureMonitor},
    proxy,
};

#[derive(RustEmbed, Default)]
#[folder = "lib/sh/azure/"]
#[include = "setup-guest-storage.sh"]
pub struct RemoteRegistry;

impl RemoteRegistry {
    pub async fn unpack_resources(tc: &ThunkContext) {
        Resources("")
            .unpack_resource::<RemoteRegistry>(tc, &String::from("setup-guest-storage.sh"))
            .await;
    }
}

impl Plugin for RemoteRegistry {
    fn symbol() -> &'static str {
        "remote_registry"
    }

    fn description() -> &'static str {
        "Sets up scripts to setup a remote registry, adds `TENANT`, `WORK_DIR` to .env properties"
    }

    fn compile(parser: &mut lifec::prelude::AttributeParser) {
        parser.add_custom_with("remote_guest", |p, _| {
            let entity = p.last_child_entity().expect("should have a last entity");

            p.define_child(entity, "enable_remote", true);
        });
    }

    fn call(context: &mut lifec::prelude::ThunkContext) -> Option<lifec::prelude::AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async {
                Self::unpack_resources(&tc).await;

                let workspace = tc.workspace().expect("should have a workspace").clone();
                let work_dir = workspace
                    .work_dir()
                    .to_str()
                    .expect("should be a string")
                    .to_string();
                let tenant = workspace.get_tenant().expect("should have tenant");

                if let Some(account_name) = tc.find_symbol("remote_registry") {
                    if !account_name.is_empty() {
                        tc.with_symbol("ACCOUNT_NAME", account_name);
                    } else if let Some(account_name) = std::env::var("ACCOUNT_NAME").ok() {
                        tc.status("Using account name from env variable").await;
                        tc.with_symbol("ACCOUNT_NAME", account_name);
                    }
                }

                tc.with_symbol("env", "TENANT")
                    .with_symbol("env", "WORK_DIR")
                    .with_symbol("env", "ACCOUNT_NAME")
                    .with_symbol("TENANT", tenant)
                    .with_symbol("WORK_DIR", &work_dir);

                if tc.is_enabled("enable_remote") {
                    let remote_registry = proxy::build_registry_proxy_guest_agent_remote(&tc).await;
                    let mut guest_context = tc.clone();
                    guest_context.enable_remote(remote_registry.subscribe());

                    if tc.enable_guest(remote_registry) {
                        event!(Level::INFO, "Guest dispatched to host");

                        let (dispatcher_cancel, dispatcher_source) = oneshot::channel();
                        let mut dispatcher_context = guest_context.clone();
                        let dispatcher_context = dispatcher_context.with_symbol(
                            "azure_dispatcher",
                            tc.find_symbol("ACCOUNT_NAME")
                                .expect("should have an account name"),
                        );
                        let dispatcher = plugins::await_plugin::<AzureDispatcher>(
                            dispatcher_source,
                            dispatcher_context,
                            |tc| Some(tc),
                        );

                        let (monitor_cancel, monitor_source) = oneshot::channel();
                        let mut monitor_context = guest_context.clone();
                        let monitor_context = monitor_context.with_symbol(
                            "azure_monitor",
                            tc.find_symbol("ACCOUNT_NAME")
                                .expect("should have an account name"),
                        );
                        let monitor = plugins::await_plugin::<AzureMonitor>(
                            monitor_source,
                            monitor_context,
                            |tc| Some(tc),
                        );

                        return select! {
                            tc = dispatcher => {
                                monitor_cancel.send(()).ok();
                                tc
                            },
                            tc = monitor => {
                                dispatcher_cancel.send(()).ok();
                                tc
                            },
                            _ = cancel_source => {
                                monitor_cancel.send(()).ok();
                                dispatcher_cancel.send(()).ok();
                                None
                            },
                        };
                    }
                }

                Some(tc)
            }
        })
    }
}

impl BlockObject for RemoteRegistry {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default()
            .optional("remote_registry")
            .optional("enable_remote")
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(RemoteRegistry::as_custom_attr())
    }
}
