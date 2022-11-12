use std::collections::HashMap;
use lifec::{
    engine::Performance,
    prelude::{BlockObject, BlockProperties, Journal, NodeStatus, Plugin},
    state::AttributeIndex,
};
use specs::{Entity, LazyUpdate, WorldExt};
use tokio::sync::oneshot::error::TryRecvError;

use super::{PollingRate, get_interval};

/// Plugin to monitor perf/status data from a remote agent,
///
#[derive(Default)]
pub struct AzureMonitor;

impl Plugin for AzureMonitor {
    fn symbol() -> &'static str {
        "azure_monitor"
    }

    fn description() -> &'static str {
        "Monitors a status, performance, etc from a store being updated by a remote agent"
    }

    fn compile(parser: &mut lifec::prelude::AttributeParser) {
        parser.with_custom::<PollingRate>();
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

                    let mut interval = get_interval(&tc);
                    while let Err(TryRecvError::Empty) = cancel_source.try_recv() {
                        if store.commit(&prefix).await && store.fetch(&prefix).await {
                            if let Some(remote) = tc.remote() {
                                let remote = remote.remote.borrow();
                                let lazy_updates = remote.as_ref().read_resource::<LazyUpdate>();

                                let performance = store.objects::<Performance>();
                                lazy_updates.exec_mut(move |world| {
                                    world.insert(Some(performance));
                                });

                                let statuses = store.objects::<NodeStatus>();
                                lazy_updates.exec_mut(|world| {
                                    let mut map = HashMap::<Entity, NodeStatus>::default();
                                    for status in statuses {
                                        map.insert(status.entity(), status);
                                    }

                                    world.insert(Some(map));
                                });

                                if let Some(journal) = store.objects::<Journal>().first() {
                                    let journal = journal.clone();
                                    lazy_updates.exec_mut(move |world| {
                                        world.insert(journal);
                                    });
                                }

                                store.take_encoder::<NodeStatus>();
                                store.take_encoder::<Journal>();
                                store.take_encoder::<Performance>();
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

impl BlockObject for AzureMonitor {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
