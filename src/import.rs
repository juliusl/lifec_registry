use lifec::{Plugin, BlockObject, BlockProperties, AttributeIndex, Process, Resources};
use rust_embed::RustEmbed;
use tokio::select;
use tracing::{event, Level};


/// Plugin to handle importing a public source image to a private repo
/// 
#[derive(Default, RustEmbed)]
#[folder = "lib/sh/"]
#[include = "import.sh"]
pub struct Import;

impl Plugin for Import {
    fn symbol() -> &'static str {
        "import"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|cancel_source|{
            let mut tc = context.clone();
            async {
                Resources("")
                    .unpack_resource::<Import>(&tc, &String::from("import.sh"))
                    .await;
                
                event!(Level::TRACE, "Unpacked script");

                if let (Some(import), Some(user), Some(token), Some(registry_name), Some(registry_host), Some(repo), Some(reference)) = (
                    tc.search().find_symbol("import"),
                    tc.search().find_text("user"),
                    tc.search().find_text("token"),
                    tc.search().find_symbol("registry_name"),
                    tc.search().find_symbol("registry_host"),
                    tc.search().find_symbol("repo"),
                    tc.search().find_symbol("reference")
                ) {
                    event!(Level::DEBUG, "Preparing a registry-env for import process");
                    tc.state_mut()
                        .with_symbol("process", "sh import.sh")
                        .with_symbol("env", "REGISTRY_NAME")
                        .with_symbol("env", "REGISTRY_HOST")
                        .with_symbol("env", "REGISTRY_USER")
                        .with_symbol("env", "REGISTRY_TOKEN")
                        .with_symbol("env", "REPO")
                        .with_symbol("env", "REFERENCE")
                        .with_symbol("env", "SOURCE")
                        .with_symbol("REGISTRY_NAME", &registry_name)
                        .with_symbol("REGISTRY_HOST", &registry_host)
                        .with_symbol("REGISTRY_USER", &user)
                        .with_symbol("REGISTRY_TOKEN", &token)
                        .with_symbol("REPO", &repo)
                        .with_symbol("SOURCE", &import)
                        .with_symbol("REFERENCE", &reference);

                        let (task, cancel) = Process::call(&tc).expect("Should start");
                        select! {
                            tc = task => {
                                if let Some(mut tc) = tc.ok() {
                                    event!(Level::DEBUG, "Finished importing - {import} -> {registry_name}.{registry_host}/{repo}:{reference}");
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

impl BlockObject for Import {
    fn query(&self) -> lifec::BlockProperties {
        BlockProperties::default()
            .require("import")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

