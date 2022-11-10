use lifec::{
    prelude::{BlockObject, BlockProperties, NodeCommand, Plugin},
    state::AttributeIndex,
};

use std::time::Duration;
use tokio::{sync::oneshot::error::TryRecvError, time::MissedTickBehavior};
use tracing::{event, Level};

/// Plugin to process an azure guest,
///
#[derive(Default)]
pub struct AzureGuest;

impl Plugin for AzureGuest {
    fn symbol() -> &'static str {
        "azure_guest"
    }

    fn description() -> &'static str {
        "Listens for node commands to dispatch"
    }

    fn caveats() -> &'static str {
        "Does not keep track of commands it has dispatched, but will only dispatch if the store being fetched has a different etag then the last store fetched."
    }

    fn call(context: &mut lifec::prelude::ThunkContext) -> Option<lifec::prelude::AsyncContext> {
        context.task(|mut cancel_source| {
            let tc = context.clone();
            async move {
                if let Some(account) = tc.find_symbol("azure_guest") {
                    let workspace = tc.workspace().expect("should have a workspace");
                    let container = workspace.get_tenant().expect("should have a tenant");
                    let prefix = workspace
                        .get_path()
                        .cloned()
                        .unwrap_or(String::from("default_guest"));

                    let mut commands = reality_azure::Store::login_azcli(account, format!("{container}-guest")).await;
                    commands.register::<NodeCommand>("node_commands");

                    let mut interval = tokio::time::interval(Duration::from_millis(800));
                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
                    while let Err(TryRecvError::Empty) = cancel_source.try_recv() {
                        if commands.take(&prefix, None).await {
                            for command in commands.objects::<NodeCommand>() {
                                tc.dispatch_node_command(command.clone());
                                event!(Level::TRACE, "Dispatched command {}", command);
                            }
                        } else {
                            interval.tick().await;
                        }
                    }
                }

                Some(tc)
            }
        })
    }
}

impl BlockObject for AzureGuest {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
