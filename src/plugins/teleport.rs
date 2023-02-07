use hyper::Body;
use hyper::Method;
use hyper::Request;
use lifec::prelude::{
    AsyncContext, AttributeIndex, BlockObject, BlockProperties, CustomAttribute,
    Plugin, ThunkContext,
};
use tracing::event;
use tracing::Level;
use tracing::info;
use tracing::warn;

use crate::ImageIndex;
use crate::ProxyTarget;
use crate::ReferrersList;

/// Plugin to handle swapping out the manifest resolution to a teleportable image
///
#[derive(Default)]
pub struct Teleport;

impl Plugin for Teleport {
    fn symbol() -> &'static str {
        "teleport"
    }

    fn description() -> &'static str {
        "Checks to see if the cached response has a referrer's api response. If so, checks for a teleport link and follows the link to resolve w/ a stremable manifest."
    }

    fn caveats() -> &'static str {
        "If there is no manifest, this plugin will GET the manifest w/ the original resolved digest"
    }

    fn call(context: &mut ThunkContext) -> Option<AsyncContext> {
        let body = context
            .take_response()
            .and_then(|r| Some(r.into_body()))
            .expect("should have body");

        context.task(|_| {
            let mut tc = context.clone();
            async move {
                match hyper::body::to_bytes::<Body>(body).await {
                    Ok(bytes) => match serde_json::from_slice::<ImageIndex>(&bytes) {
                        Ok(list) => {
                            let list = ReferrersList { referrers: list.manifests };
                            let streamable = list.find_streamable_descriptors();
                            let digest = if let Some(streamable_desc) = streamable.first() {
                                info!("Streamable descriptor was found");
                                streamable_desc.digest.to_string()
                            } else {
                                warn!("No streamable descriptor was found, {:?} {:?}", list, streamable);
                                let digest = tc.search().find_symbol("digest").expect("should have a digest property");
                                digest
                            };

                            let mut ptc = tc.clone();
                            ptc.replace_symbol("digest", digest);

                            let manifest_uri = ProxyTarget::try_from(&ptc).expect("should have a proxy target");
                            let manifest = tc.client()
                                .expect("should have client")
                                .request(
                                    Request::builder()
                                        .method(Method::HEAD)
                                        .uri(manifest_uri.manifest_url())
                                        .header("Accept", tc.search().find_symbol("accept").expect("should have accept header"))
                                        .header("Authorization", tc.search().find_symbol("Authorization").expect("should have authorization"))
                                        .body(Body::empty())
                                        .expect("should be able to create request")
                                        .into()
                                    ).await.expect("should have response");

                            tc.cache_response(
                                manifest
                            )
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Error deserializing referrer's list, {err}");
                        }
                    },
                    Err(err) => {
                        event!(
                            Level::ERROR,
                            "Error reading body from cached response, {err}"
                        );
                    }
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }
}

impl BlockObject for Teleport {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Teleport::as_custom_attr())
    }
}
