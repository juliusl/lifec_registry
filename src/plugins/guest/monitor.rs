use std::{ops::Deref, time::Duration};

use lifec::{
    engine::{Performance, Runner},
    prelude::{Journal, NodeStatus, Plugin, BlockObject, BlockProperties},
    state::AttributeIndex,
};
use specs::{Join, WorldExt};
use tokio::{sync::oneshot::error::TryRecvError, time::MissedTickBehavior};
use tracing::{event, Level};

/// Plugin that monitors guest state and uploads when changes occur,
///
#[derive(Default)]
pub struct AzureMonitor;

impl Plugin for AzureMonitor {
    fn symbol() -> &'static str {
        "azure_monitor"
    }

    fn description() -> &'static str {
        "This plugin will watch and wait for changes to the remote_protocol object in it's thunk context, and then encode and upload state"
    }

    fn call(context: &mut lifec::prelude::ThunkContext) -> Option<lifec::prelude::AsyncContext> {
        context.task(|mut cancel_source| {
            let tc = context.clone();
            async move {
                if let Some(account) = tc.find_symbol("azure_monitor") {
                    let workspace = tc.workspace().expect("should have a workspace");
                    let container = workspace.get_tenant().expect("should have a tenant");
                    let prefix = workspace
                        .get_path()
                        .cloned()
                        .unwrap_or(String::from("default_guest"));

                    let mut store = reality_azure::Store::login_azcli(account, container).await;
                    store.register::<Journal>("journal");
                    store.register::<NodeStatus>("node_status");
                    store.register::<Performance>("performance");

                    let mut interval = tokio::time::interval(Duration::from_millis(800));
                    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
                    
                    while let Err(TryRecvError::Empty) = cancel_source.try_recv() {
                        if let Some(remote_protocol) = tc.remote().as_ref() {
                            let mut remote = remote_protocol.remote.clone();

                            match remote.changed().await {
                                Ok(_) => {
                                    let state = remote.borrow();
                                    let mut runner = state.as_ref().system_data::<Runner>();
                                    if let Some(encoder) = store.encoder_mut::<Performance>() {
                                        for (_, perf) in runner.take_performance() {
                                            encoder.encode(&perf, state.as_ref());
                                        }
                                    }
                                    let journal = state.as_ref().read_resource::<Journal>();
                                    if let Some(encoder) = store.encoder_mut::<Journal>() {
                                        encoder.encode(journal.deref(), state.as_ref());
                                    }

                                    let status = state.as_ref().read_component::<NodeStatus>();
                                    if let Some(encoder) = store.encoder_mut::<NodeStatus>() {
                                        for status in status.join() {
                                            encoder.encode(status, state.as_ref());
                                        }
                                    }
                                }
                                Err(err) => {
                                    event!(
                                        Level::ERROR,
                                        "Error waiting for change in remote protocol, {err}"
                                    );
                                }
                            }

                            store.upload(&prefix).await;

                            if let Some(encoder) = store.encoder_mut::<Journal>() {
                                encoder.clear();
                            }

                            if let Some(encoder) = store.encoder_mut::<NodeStatus>() {
                                encoder.clear();
                            }

                            if let Some(encoder) = store.encoder_mut::<Performance>() {
                                encoder.clear();
                            }
                        }
                    }
                }

                Some(tc)
            }
        })
    }
}

impl BlockObject for AzureMonitor {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
