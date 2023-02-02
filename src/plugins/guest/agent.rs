use std::{ops::Deref, collections::HashMap};

use lifec::{
    engine::{Performance, Runner},
    prelude::{Journal, NodeStatus, Plugin, BlockObject, BlockProperties},
    state::AttributeIndex, debugger::Debugger,
};
use specs::{Join, WorldExt, Entity, LazyUpdate};
use tokio::sync::oneshot::error::TryRecvError;

use super::{PollingRate, get_interval};

/// Plugin that monitors guest state and uploads when changes occur,
///
#[derive(Default)]
pub struct AzureAgent;

impl Plugin for AzureAgent {
    fn symbol() -> &'static str {
        "azure_agent"
    }

    fn description() -> &'static str {
        "This plugin will watch and wait for changes to the remote_protocol object in it's thunk context, and then encode and upload state"
    }

    fn compile(parser: &mut lifec::prelude::AttributeParser) {
        parser.with_custom::<PollingRate>();
    }

    fn call(context: &mut lifec::prelude::ThunkContext) -> Option<lifec::prelude::AsyncContext> {
        context.task(|mut cancel_source| {
            let tc = context.clone();
            async move {
                if let Some(account) = tc.find_symbol("azure_agent") {
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
                    store.register::<Debugger>("debugger");

                    let mut interval = get_interval(&tc);
                    while let Err(TryRecvError::Empty) = cancel_source.try_recv() {
                        if let Some(remote_protocol) = tc.remote().as_ref() {
                            let state = remote_protocol.remote.borrow();
                            let mut runner = state.as_ref().system_data::<Runner>();
                            if let Some(encoder) = store.encoder_mut::<Performance>() {
                                encoder.clear();
                                let mut map = HashMap::<(Entity, Entity), Performance>::default();
                                for (_, perf) in runner.take_performance() {
                                    map.insert((perf.from, perf.to), perf);
                                }

                                for (_, perf) in map {
                                    encoder.encode(&perf, state.as_ref());
                                }
                            }

                            let journal = state.as_ref().read_resource::<Journal>();
                            if let Some(encoder) = store.encoder_mut::<Journal>() {
                                encoder.clear();
                                encoder.encode(journal.deref(), state.as_ref());
                            }

                            let status = state.as_ref().read_component::<NodeStatus>();
                            if let Some(encoder) = store.encoder_mut::<NodeStatus>() {
                                encoder.clear();
                                for status in status.join() {
                                    encoder.encode(status, state.as_ref());
                                }
                            }

                            let lazy_update = state.as_ref().read_resource::<LazyUpdate>();
                            lazy_update.exec_mut(|world| {
                                let mut debugger = world.read_resource::<Option<Debugger>>().deref().clone();
                                if let Some(debugger) = debugger.take() {
                                    world.insert(debugger);
                                }
                            });
                            
                            let debugger = state.as_ref().try_fetch::<Debugger>();
                            if let Some(debugger) = debugger.as_ref() {
                                if let Some(encoder) = store.encoder_mut::<Debugger>() {
                                    encoder.clear();
                                    encoder.encode(debugger.deref(), state.as_ref());
                                }
                            }
                        }

                        store.upload(&prefix).await;
                        
                        interval.tick().await;
                    }
                }

                Some(tc)
            }
        })
    }
}

impl BlockObject for AzureAgent {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
