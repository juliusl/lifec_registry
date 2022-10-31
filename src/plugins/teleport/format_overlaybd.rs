use lifec::prelude::{ThunkContext, AsyncContext, CustomAttribute};
use lifec::prelude::{BlockObject, BlockProperties, AttributeIndex, Plugin, Resources, Process};
use rust_embed::RustEmbed;
use tokio::select;
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
        "format-overlaybd"
    }

    fn call(context: &mut ThunkContext) -> Option<AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                if !tc.search().find_bool("requires-conversion").unwrap_or_default() {
                    event!(Level::DEBUG, "Skipping conversion");
                    tc.copy_previous();
                    return Some(tc);
                }

                Resources("")
                    .unpack_resource::<FormatOverlayBD>(&tc, &String::from("format-overlaybd.sh"))
                    .await;
                
                event!(Level::DEBUG, "Unpacked script");

                let registry = tc.workspace().expect("should have a workspace").get_tenant().expect("should have a tenant").clone();
                let registry_host = tc.workspace().expect("should have a workspace").get_host().clone();

                if let (Some(user), Some(token), Some(repo), Some(reference)) = (
                    tc.search().find_text("user"),
                    tc.search().find_text("token"),
                    tc.search().find_symbol("repo"),
                    tc.search().find_symbol("reference")
                ) {
                    event!(Level::DEBUG, "Preparing a registry-env for format process");
                    tc.state_mut()
                        .with_symbol("process", "sh format-overlaybd.sh")
                        .with_symbol("env", "REGISTRY_NAME")
                        .with_symbol("env", "REGISTRY_HOST")
                        .with_symbol("env", "REGISTRY_USER")
                        .with_symbol("env", "REGISTRY_TOKEN")
                        .with_symbol("env", "REPO")
                        .with_symbol("env", "REFERENCE")
                        .with_symbol("REGISTRY_NAME", &registry)
                        .with_symbol("REGISTRY_HOST", &registry_host)
                        .with_symbol("REGISTRY_USER", &user)
                        .with_symbol("REGISTRY_TOKEN", &token)
                        .with_symbol("REPO", &repo)
                        .with_symbol("REFERENCE", &reference);

                        let (task, cancel) = Process::call(&mut tc).expect("Should start");
                        select! {
                            tc = task => {
                                if let Some(mut tc) = tc.ok() {
                                    event!(Level::DEBUG, "Finished formatting - {registry}.{registry_host}/{repo}:{reference} -> {reference}-overlaybd");
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

impl BlockObject for FormatOverlayBD {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

