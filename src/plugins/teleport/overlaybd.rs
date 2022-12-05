use lifec::prelude::{AttributeIndex, BlockObject, BlockProperties, Plugin, Process, Resources};
use lifec::prelude::{AsyncContext, CustomAttribute, ThunkContext};
use rust_embed::RustEmbed;
use tracing::event;
use tracing::Level;

/// Plugin for formatting overlaybd,
///
#[derive(RustEmbed, Default)]
#[folder = "lib/sh/"]
#[include = "format-overlaybd.sh"]
pub struct FormatOverlayBD;

impl Plugin for FormatOverlayBD {
    fn symbol() -> &'static str {
        "format_overlaybd"
    }

    fn call(context: &mut ThunkContext) -> Option<AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                Resources("")
                    .unpack_resource::<FormatOverlayBD>(&tc, &String::from("format-overlaybd.sh"))
                    .await;

                event!(Level::DEBUG, "Preparing a registry-env for format process");
                tc.state_mut()
                    .with_symbol("process", "sh format-overlaybd.sh")
                    .with_symbol("env", "REGISTRY_TENANT")
                    .with_symbol("env", "REGISTRY_HOST")
                    .with_symbol("env", "REGISTRY_USER")
                    .with_symbol("env", "REGISTRY_TOKEN")
                    .with_symbol("env", "REGISTRY_REPO")
                    .with_symbol("env", "REFERENCE");

                lifec::plugins::await_plugin::<Process>(cancel_source, &mut tc, |mut tc| {
                    event!(Level::DEBUG, "Finished formatting - overlaybd");
                    tc.copy_previous();
                    Some(tc)
                })
                .await
            }
        })
    }
}

impl BlockObject for FormatOverlayBD {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
