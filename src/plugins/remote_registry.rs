use lifec::{
    prelude::{BlockObject, BlockProperties, Plugin, ThunkContext},
    resources::Resources,
    state::AttributeIndex,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed, Default)]
#[folder = "lib/sh/"]
#[include = "fetch-guest-commands.sh"]
#[include = "fetch-guest-state.sh"]
#[include = "monitor-guest.sh"]
#[include = "send-guest-commands.sh"]
#[include = "send-guest-storage.sh"]
pub struct RemoteRegistry;

impl RemoteRegistry {
    pub async fn unpack_resources(tc: &ThunkContext) {
        Resources("")
            .unpack_resource::<RemoteRegistry>(tc, &String::from("fetch-guest-state.sh"))
            .await;
        Resources("")
            .unpack_resource::<RemoteRegistry>(tc, &String::from("fetch-guest-commands.sh"))
            .await;
        Resources("")
            .unpack_resource::<RemoteRegistry>(tc, &String::from("monitor-guest.sh"))
            .await;
        Resources("")
            .unpack_resource::<RemoteRegistry>(tc, &String::from("send-guest-commands.sh"))
            .await;
        Resources("")
            .unpack_resource::<RemoteRegistry>(tc, &String::from("send-guest-storage.sh"))
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

    fn call(context: &mut lifec::prelude::ThunkContext) -> Option<lifec::prelude::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async {
                Self::unpack_resources(&tc).await;

                let workspace = tc.workspace().expect("should have a workspace").clone();

                tc.with_symbol("env", "TENANT")
                    .with_symbol("env", "WORK_DIR")
                    .with_symbol(
                        "TENANT",
                        workspace.get_tenant().expect("should have tenant"),
                    )
                    .with_symbol(
                        "WORK_DIR",
                        workspace
                            .work_dir()
                            .to_str()
                            .expect("should be a string")
                            .to_string(),
                    );

                Some(tc)
            }
        })
    }
}

impl BlockObject for RemoteRegistry {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(RemoteRegistry::as_custom_attr())
    }
}
