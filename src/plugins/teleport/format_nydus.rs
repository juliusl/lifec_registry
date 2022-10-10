use lifec::{Plugin, BlockObject, BlockProperties, AttributeIndex, Resources, Process};
use rust_embed::RustEmbed;
use tokio::select;
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
        "format-nydus"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                if !tc.search().find_bool("requires-conversion").unwrap_or_default() {
                    event!(Level::DEBUG, "Skipping conversion");
                    tc.copy_previous();
                    return Some(tc);
                }

                Resources("")
                    .unpack_resource::<FormatNydus>(&tc, &String::from("format-nydus.sh"))
                    .await;
                
                event!(Level::DEBUG, "Unpacked script");

                if let (Some(registry_name), Some(registry_host), Some(repo), Some(reference)) = (
                    tc.search().find_symbol("registry_name"),
                    tc.search().find_symbol("registry_host"),
                    tc.search().find_symbol("repo"),
                    tc.search().find_symbol("reference")
                ) {
                    event!(Level::DEBUG, "Preparing a registry-env for format process");
                    tc.state_mut()
                        .with_symbol("process", "sh format-nydus.sh")
                        .with_symbol("env", "REGISTRY_NAME")
                        .with_symbol("env", "REGISTRY_HOST")
                        .with_symbol("env", "REPO")
                        .with_symbol("env", "REFERENCE")
                        .with_symbol("env", "DOCKER_CONFIG")
                        .with_symbol("env", "NYDUS_INSTALL_DIR")
                        .with_symbol("REGISTRY_NAME", &registry_name)
                        .with_symbol("REGISTRY_HOST", &registry_host)
                        .with_symbol("DOCKER_CONFIG", format!(".world/{registry_host}/{registry_name}/.docker"))
                        .with_symbol("REPO", &repo)
                        .with_symbol("NYDUS_INSTALL_DIR", "/usr/local/bin")
                        .with_symbol("REFERENCE", &reference);

                        let (task, cancel) = Process::call(&tc).expect("Should start");
                        select! {
                            tc = task => {
                                if let Some(mut tc) = tc.ok() {
                                    event!(Level::DEBUG, "Finished formatting - {registry_name}.{registry_host}/{repo}:{reference} -> {reference}-nydus");
                                    tc.copy_previous();
                                    return Some(tc);
                                } else {
                                    return None;
                                }
                            }
                            _ = cancel_source => {
                                cancel.send(()).ok();
                                return None;
                            }
                        }
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }
}

impl BlockObject for FormatNydus {
    fn query(&self) -> lifec::BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

