use std::path::PathBuf;

use lifec::{
    plugins,
    prelude::{BlockObject, BlockProperties, Plugin, ThunkContext},
    resources::Resources,
    state::AttributeIndex,
};
use rust_embed::RustEmbed;
use tracing::{event, Level};

use crate::{plugins::guest::AzureDispatcher, proxy};

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
                    }
                }

                tc.with_symbol("env", "TENANT")
                    .with_symbol("env", "WORK_DIR")
                    .with_symbol("env", "ACCOUNT_NAME")
                    .with_symbol("TENANT", tenant)
                    .with_symbol("WORK_DIR", &work_dir);

                let guest_dir = PathBuf::from(&work_dir).join(".guest");
                let guest_command_dir = PathBuf::from(&work_dir).join(".guest-commands");
                match tokio::fs::create_dir_all(guest_dir).await {
                    Ok(_) => {}
                    Err(err) => {
                        event!(Level::ERROR, "Could not create .guest dir, {err}");
                    }
                }

                match tokio::fs::create_dir_all(guest_command_dir).await {
                    Ok(_) => {}
                    Err(err) => {
                        event!(Level::ERROR, "Could not create .guest-commands dir, {err}");
                    }
                }

                if tc.is_enabled("enable_remote") {
                    let remote_registry = proxy::build_registry_proxy_guest_agent_remote(&tc).await;
                    let mut guest_context = tc.clone();
                    guest_context.with_symbol(
                        "azure_dispatcher",
                        tc.find_symbol("ACCOUNT_NAME")
                            .expect("should have an account name"),
                    );

                    guest_context.enable_remote(remote_registry.subscribe());

                    if tc.enable_guest(remote_registry) {
                        event!(Level::INFO, "Guest dispatched to host");

                        return plugins::await_plugin::<AzureDispatcher>(
                            cancel_source,
                            &mut guest_context,
                            |tc| Some(tc),
                        )
                        .await;
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
