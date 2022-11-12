use lifec::{
    engine::Runner,
    prelude::{BlockObject, BlockProperties, NodeCommand, Plugin},
    state::AttributeIndex,
};
use tokio::sync::oneshot::error::TryRecvError;

use super::{get_interval, PollingRate};

/// Plugin to dispatch commands to azure storage,
///
#[derive(Default)]
pub struct AzureDispatcher;

impl Plugin for AzureDispatcher {
    fn symbol() -> &'static str {
        "azure_dispatcher"
    }

    fn description() -> &'static str {
        "Listens for changes to the remote_protocol and dispatches those changes to store"
    }

    fn compile(parser: &mut lifec::prelude::AttributeParser) {
        parser.with_custom::<PollingRate>();
    }

    fn call(context: &mut lifec::prelude::ThunkContext) -> Option<lifec::prelude::AsyncContext> {
        context.task(|mut cancel_source| {
            let tc = context.clone();
            async move {
                if let Some(account) = tc.find_symbol("azure_dispatcher") {
                    let workspace = tc.workspace().expect("should have a workspace");
                    let container = workspace.get_tenant().expect("should have a tenant");
                    let prefix = workspace
                        .get_path()
                        .cloned()
                        .unwrap_or(String::from("default_guest"));

                    let mut commands =
                        reality_azure::Store::login_azcli(account, format!("{container}-guest"))
                            .await;
                    commands.register::<NodeCommand>("node_commands");

                    let mut interval = get_interval(&tc);
                    while let Err(TryRecvError::Empty) = cancel_source.try_recv() {
                        let mut has_commands = false;

                        if let Some(remote) = tc.remote() {
                            if let Some(encoder) = commands.encoder_mut::<NodeCommand>() {
                                let state = remote.remote.borrow();
                                let mut runner = state.as_ref().system_data::<Runner>();
                                for (_, command) in runner.take_commands() {
                                    encoder.encode(&command, state.as_ref());
                                    has_commands = true;
                                }
                            }

                            if has_commands && commands.upload(&prefix).await {
                                if let Some(encoder) = commands.encoder_mut::<NodeCommand>() {
                                    encoder.clear();
                                }
                            }
                        }

                        interval.tick().await;
                    }
                }

                Some(tc)
            }
        })
    }
}

impl BlockObject for AzureDispatcher {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
