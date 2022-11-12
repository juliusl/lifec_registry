use std::{sync::Arc, collections::HashMap};

use imgui::Window;
use lifec::{prelude::{Node, NodeStatus, Appendix, State, EventStatus, Plugins}, guest::RemoteProtocol, editor::EventNode};
use specs::{Entity, WorldExt};
use tracing::{event, Level};

/// Returns a node for controlling a remote guest,
/// 
pub fn guest_control_node(entity: Entity, appendix: Arc<Appendix>, remote_protocol: RemoteProtocol) -> Node {
    Node {
        status: NodeStatus::Custom(entity),
        appendix,
        remote_protocol: Some(remote_protocol),
        edit: Some(|node, ui| {
            let mut opened = true;
            let opened = &mut opened;

            Window::new("Guest controls").opened(opened).build(ui, || {
                if let Some(rp) = node.remote_protocol.as_ref() {
                    let remote = rp.remote.borrow();
                    let state = remote.as_ref().system_data::<State>();

                    let mut status_map = HashMap::<Entity, EventStatus>::default();
                    {
                        let mut store = remote.as_ref().write_resource::<reality_azure::Store>();
                        let status = store.objects::<NodeStatus>();
                        for status in status.iter() {
                            match status {
                                NodeStatus::Event(e) => {
                                    status_map.insert(e.entity(), *e);
                                }
                                _ => {}
                            }
                        }
                    }
                    let event_nodes = { state.event_nodes() };

                    if let Some(token) = ui.begin_table("controls", 2) {
                        ui.table_setup_column("Event");
                        ui.table_setup_column("Controls");
                        ui.table_headers_row();

                        for mut node in event_nodes {
                            match node.status {
                                NodeStatus::Event(status) => {
                                    ui.table_next_row();
                                    ui.table_next_column();
                                    ui.text(format!(
                                        "{}.{}",
                                        node.appendix
                                            .control_symbol(&status.entity())
                                            .unwrap_or_default(),
                                        node.appendix.name(&status.entity()).unwrap_or_default(),
                                    ));

                                    ui.table_next_column();
                                    node.event_buttons(
                                        ui,
                                        if let Some(status) = status_map.get(&status.entity()) {
                                            *status
                                        } else {
                                            status
                                        },
                                    );
                                    if let Some(command) = node.command.take() {
                                        event!(Level::DEBUG, "Trying to dispatch, {command}");
                                        remote
                                            .as_ref()
                                            .system_data::<Plugins>()
                                            .features()
                                            .broker()
                                            .try_send_node_command(command, None)
                                            .ok();
                                    }
                                }
                                _ => {}
                            }
                        }

                        token.end();
                    }
                }
            });
            *opened
        }),
        ..Default::default()
    }
}
