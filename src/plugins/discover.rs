use std::str::from_utf8;
use lifec::{BlockObject, Plugin, AttributeIndex};
use tracing::event;
use tracing::Level;

use crate::ProxyTarget;

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

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async move {
                if let (Some(artifact_type), Some(digest)) = (
                    tc.state().find_symbol("discover"),
                    tc.search().find_symbol("digest"),
                ) {
                    event!(Level::DEBUG, "Discovering {artifact_type}");
                    if let Some(proxy_target) = ProxyTarget::try_from(&tc).ok() {
                        let api = tc
                            .state()
                            .find_symbol("referrers_api")
                            .unwrap_or("_oras/artifacts/referrers".to_string());

                        let referrers_api = format!(
                            "https://{}/v2/{}/{api}?digest={digest}&artifactType={artifact_type}",
                            proxy_target.namespace,
                            proxy_target.repo,
                        );
                        event!(
                            Level::DEBUG,
                            "Making referrers call for {artifact_type}\n{referrers_api}"
                        );
                        let req = proxy_target
                            .start_request()
                            .expect("should be able to create a request")
                            .uri_str(referrers_api.as_str())
                            .finish();

                        match proxy_target.send_request(req).await {
                            Some(response) => {
                                match hyper::body::to_bytes(response.into_body()).await {
                                    Ok(data) => {
                                        event!(Level::TRACE, "{:#?}", from_utf8(&data).ok());
                                        tc.state_mut().add_binary_attr(&artifact_type, data)
                                    }
                                    Err(err) => event!(
                                        Level::ERROR,
                                        "Could not read referrers response body {err}"
                                    ),
                                }
                            }
                            None => {
                                event!(Level::ERROR, "Could not send request for referrers api")
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
    fn query(&self) -> lifec::BlockProperties {
        lifec::BlockProperties::default()
            .require("discover")
            .require("digest")
            .require("repo")
            .require("ns")
            .require("access_token")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
