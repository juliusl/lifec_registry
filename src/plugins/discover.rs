use lifec::prelude::{AsyncContext, BlockProperties, CustomAttribute, ThunkContext};
use lifec::prelude::{AttributeIndex, BlockObject, Plugin};
use tokio::select;
use tracing::event;
use tracing::Level;

/// Plugin for calling the referrer's api and adding the result to state,
///
#[derive(Default)]
pub struct Discover;

impl Plugin for Discover {
    fn symbol() -> &'static str {
        "discover"
    }

    fn description() -> &'static str {
        "Uses the registry referrer's api to find artifacts by type and subject digest"
    }

    fn call(context: &ThunkContext) -> Option<AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                if let (Some(artifact_type), Some(digest), Some(namespace), Some(repo)) = (
                    tc.state().find_symbol("discover"),
                    tc.search().find_symbol("digest"),
                    tc.search().find_symbol("REGISTRY_NAMESPACE"),
                    tc.search().find_symbol("REGISTRY_REPO")
                ) {
                    event!(Level::DEBUG, "Discovering {artifact_type}");
                    let api = tc
                        .state()
                        .find_symbol("referrers_api")
                        .unwrap_or("_oras/artifacts/referrers".to_string());

                    let referrers_api = format!(
                        "https://{}/v2/{}/{api}?digest={digest}&artifactType={artifact_type}",
                        namespace, repo,
                    );
                    event!(
                        Level::DEBUG,
                        "Making referrers call for {artifact_type}\n{referrers_api}"
                    );

                    tc.state_mut().replace_symbol("request", referrers_api);

                    if let Some((task, cancel)) = lifec::plugins::Request::call(&tc) {
                        select! {
                            result = task => {
                                match result {
                                    Ok(mut context) => {
                                        context.copy_previous();
                                        return Some(context);
                                    },
                                    Err(err) => {
                                        event!(Level::ERROR, "Error calling plugin, {err}");
                                    },
                                }
                            },
                            _ = cancel_source => {
                                cancel.send(()).ok();
                                event!(Level::WARN, "Cancelling request");
                            }
                        }
                    }
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }
}

impl BlockObject for Discover {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
            .require("discover")
            .require("digest")
            .require("REGISTRY_NAMESPACE")
            .require("REGISTRY_REPO")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
