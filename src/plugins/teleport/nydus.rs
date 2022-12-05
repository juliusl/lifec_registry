use lifec::prelude::{Plugin, BlockObject, BlockProperties, AttributeIndex, Resources, Process, AsyncContext, ThunkContext, CustomAttribute};
use rust_embed::RustEmbed;
use tracing::{event, Level};


/// Plugin for formatting nydus,
/// 
/// TODO -- Needs a DRY pass w/ format_overlaybd, 
/// 
#[derive(RustEmbed, Default)]
#[folder = "lib/sh/"]
#[include = "format-nydus.sh"]
pub struct FormatNydus;

impl Plugin for FormatNydus {
    fn symbol() -> &'static str {
        "format_nydus"
    }

    fn call(context: &mut ThunkContext) -> Option<AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                {
                    Resources("")
                    .unpack_resource::<FormatNydus>(&tc, &String::from("format-nydus.sh"))
                    .await;
                }
                
                event!(Level::DEBUG, "Unpacked script");
                let workspace = tc.workspace().cloned().expect("should have a workspace");

                event!(Level::DEBUG, "Preparing a registry-env for format process");
                tc.state_mut()
                    .with_symbol("process", "sh format-nydus.sh")
                    .with_symbol("env", "REGISTRY_NAME")
                    .with_symbol("env", "REGISTRY_HOST")
                    .with_symbol("env", "REGISTRY_REPO")
                    .with_symbol("env", "REFERENCE")
                    .with_symbol("env", "DOCKER_CONFIG")
                    .with_symbol("env", "NYDUS_INSTALL_DIR")
                    .with_symbol("DOCKER_CONFIG", workspace.work_dir().join(".docker").to_string_lossy())
                    .with_symbol("NYDUS_INSTALL_DIR", "/usr/local/bin");

                lifec::plugins::await_plugin::<Process>(cancel_source, &mut tc, move |mut result| {
                    event!(Level::DEBUG, "Finished formatting nydus");
                    result.copy_previous();
                    Some(result)
                }).await
            }
        })
    }
}

impl BlockObject for FormatNydus {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

