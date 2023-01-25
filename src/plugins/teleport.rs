use hyper::Body;
use lifec::prelude::{
    AsyncContext, AttributeIndex, AttributeParser, BlockObject, BlockProperties, CustomAttribute,
    Plugin, ThunkContext, Value,
};
use tracing::event;
use tracing::Level;

use crate::content::{ArtifactManifest, Descriptor, ImageManifest, ReferrersList};
use crate::ProxyTarget;

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
                    Ok(bytes) => match serde_json::from_slice::<ReferrersList>(&bytes) {
                        Ok(list) => {
                            event!(Level::DEBUG, "Got referrer's response");
                            if list.referrers.is_empty() {
                                event!(Level::DEBUG, "No referrer's found, Requires link.");
                                tc.state_mut().with_bool("requires-conversion", true);

                            } else if let Some(referrer) = list.referrers.first() {
                                return Teleport::resolve_teleportable_manifest(&tc, referrer)
                                    .await;
                            }
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

    fn compile(parser: &mut AttributeParser) {
        parser.add_custom(CustomAttribute::new_with("from", |p, content| {
            if let Some(last_entity) = p.last_child_entity() {
                p.define_child(last_entity, "from", Value::Symbol(content));

                p.add_custom(CustomAttribute::new_with("to", |p, content| {
                    if let Some(last_entity) = p.last_child_entity() {
                        p.define_child(last_entity, "to", Value::Symbol(content));
                    }
                }));
            }
        }));
    }
}

impl Teleport {
    /// Resolves the artifact manifest from a descriptor,
    ///
    async fn resolve_teleportable_manifest(
        tc: &ThunkContext,
        descriptor: &Descriptor,
    ) -> Option<ThunkContext> {
        if let Some(proxy_target) = ProxyTarget::try_from(tc).ok() {
            if let Some(artifact) = proxy_target.request_content(descriptor).await {
                if let Some(artifact_manifest) =
                    serde_json::from_slice::<ArtifactManifest>(artifact.as_slice()).ok()
                {
                    // TODO -- Check env variable for what snapshotter is being used at the moment
                    if let Some(streamable_manifest) = artifact_manifest.blobs.iter().find(|b| {
                        // Converted overlaybd is this type
                        b.media_type == "application/vnd.docker.distribution.manifest.v2+json"
                            // Converted nydus is this type
                            || b.media_type == "application/vnd.oci.image.manifest.v1+json"
                    }) {
                        if let Some(response) =
                            proxy_target.request_content(streamable_manifest).await
                        {
                            if let Some(_) =
                                serde_json::from_slice::<ImageManifest>(response.as_slice()).ok()
                            {
                                // Format the thunk context
                                let mut tc = tc.commit();

                                tc.with_binary("body", response)
                                    .with_symbol(
                                        "content-type",
                                        streamable_manifest.media_type.to_string(),
                                    )
                                    .with_symbol("digest", streamable_manifest.digest.to_string())
                                    .with_int("status-code", 200);

                                return Some(tc);
                            }
                        }
                    }
                }
            }
        }
        None
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
