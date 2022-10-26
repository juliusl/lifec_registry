use lifec::prelude::{Plugin, BlockObject, BlockProperties, AttributeIndex, Resources, Process, AsyncContext, ThunkContext, CustomAttribute};
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

    fn call(context: &ThunkContext) -> Option<AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                if !tc.search().find_bool("requires-conversion").unwrap_or_default() {
                    event!(Level::DEBUG, "Skipping conversion");
                    tc.copy_previous();
                    return Some(tc);
                }

                {
                    Resources("")
                    .unpack_resource::<FormatNydus>(&tc, &String::from("format-nydus.sh"))
                    .await;
                }
                
                event!(Level::DEBUG, "Unpacked script");
                let workspace = tc.workspace().cloned().expect("should have a workspace");
                let host = workspace.get_host();
                let tenant = workspace.get_tenant().expect("should have a tenant");
                let repo = workspace.get_path().expect("should have a path/repo");
                let tag = workspace.iter_tags().next().expect("should have a tag/reference");

                event!(Level::DEBUG, "Preparing a registry-env for format process");
                tc.state_mut()
                    .with_symbol("process", "sh format-nydus.sh")
                    .with_symbol("env", "REGISTRY_NAME")
                    .with_symbol("env", "REGISTRY_HOST")
                    .with_symbol("env", "REPO")
                    .with_symbol("env", "REFERENCE")
                    .with_symbol("env", "DOCKER_CONFIG")
                    .with_symbol("env", "NYDUS_INSTALL_DIR")
                    .with_symbol("REGISTRY_NAME", &tenant)
                    .with_symbol("REGISTRY_HOST", &host)
                    .with_symbol("DOCKER_CONFIG", format!(".world/{host}/{tenant}/.docker"))
                    .with_symbol("REPO", &repo)
                    .with_symbol("NYDUS_INSTALL_DIR", "/usr/local/bin")
                    .with_symbol("REFERENCE", &tag);

                let (task, cancel) = Process::call(&tc).expect("Should start");
                select! {
                    tc = task => {
                        if let Some(mut tc) = tc.ok() {
                            event!(Level::DEBUG, "Finished formatting - {tenant}.{host}/{repo}:{tag} -> {tag}-nydus");
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

