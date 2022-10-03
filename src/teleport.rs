use std::str::from_utf8;
use lifec::{Plugin, BlockObject, AttributeIndex};
use tracing::event;
use tracing::Level;

/// Plugin to handle swapping out the manifest resolution to a teleportable image
/// 
#[derive(Default)]
pub struct Teleport;

impl Plugin for Teleport {
    fn symbol() -> &'static str {
        "teleport"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async move { 
                if let Some(teleport_format) = tc.state().find_symbol("teleport") {
                    event!(Level::DEBUG, "Teleport format {teleport_format}");

                    if let Some(artifact) = tc.state().find_binary("dadi.image.v1") {
                        let artifact = from_utf8(artifact.as_slice()).expect("should deserialize");

                        event!(Level::DEBUG, "{}", &artifact);
                    }
                }

                tc.copy_previous();

                Some(tc) 
            }
        })
    }
}

impl BlockObject for Teleport {
    fn query(&self) -> lifec::BlockProperties {
        lifec::BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Teleport::as_custom_attr())
    }
}
