use lifec::prelude::{
    AsyncContext, AttributeIndex, AttributeParser, BlockObject, BlockProperties, CustomAttribute,
    Plugin, ThunkContext, Value,
};
use tracing::event;
use tracing::Level;

use crate::content::{ArtifactManifest, Descriptor, ImageManifest, ReferrersList};
use crate::ProxyTarget;

mod format_overlaybd;
pub use format_overlaybd::FormatOverlayBD;

mod format_nydus;
pub use format_nydus::FormatNydus;

/// Plugin to handle swapping out the manifest resolution to a teleportable image
///
#[derive(Default)]
pub struct Teleport;

impl Plugin for Teleport {
    fn symbol() -> &'static str {
        "teleport"
    }

    fn description() -> &'static str {
        "Checks to see the current image being resolved has a streamable format, if so, sets the response for the streamable format instead of the original"
    }

    fn caveats() -> &'static str {
        "Expects that the calling snapshotter is capable of using the streamable format."
    }

    fn call(context: &ThunkContext) -> Option<AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async move {
                if let Some(teleport_format) = tc.search().find_symbol("teleport") {
                    event!(Level::DEBUG, "Teleport format {teleport_format}");

                    match teleport_format.as_str() {
                        "nydus" | "overlaybd" => {
                            if let Some(artifact) = tc.search().find_binary("teleport.link.v1") {
                                if let Some(response) =
                                    serde_json::from_slice::<ReferrersList>(artifact.as_slice())
                                        .ok()
                                {
                                    event!(Level::DEBUG, "Got referrer's response");
                                    if response.referrers.is_empty() {
                                        event!(Level::DEBUG, "No referrer's found");
                                    }

                                    // Next we'll need to fetch the referrers
                                    if let Some(referrer) = response.referrers.first() {
                                        return Teleport::resolve_teleportable_manifest(
                                            &tc, referrer,
                                        )
                                        .await;
                                    }
                                } else {
                                    event!(Level::ERROR, "Could not parse referrer's response");
                                }
                            } else {
                                event!(Level::DEBUG, "Requires conversion");
                                tc.state_mut().with_bool("requires-conversion", true);
                            }
                        }
                        "manual" => {
                            if let (Some(from), Some(to)) = (
                                tc.search().find_symbol("from"),
                                tc.search().find_symbol("to"),
                            ) {
                                if let Some(digest) = tc.search().find_symbol("digest") {
                                    if digest == from {
                                        if let Some(mut proxy_target) =
                                            ProxyTarget::try_from(&tc).ok()
                                        {
                                            proxy_target.context.replace_symbol("digest", &to);
                                            if let Some((manifests, body)) =
                                                proxy_target.resolve().await
                                            {
                                                event!(
                                                    Level::DEBUG,
                                                    "Manual teleport mode, swapping {from} -> {to}"
                                                );
                                                let mut swap = ThunkContext::default();
                                                manifests.copy_to_context(&mut swap);
                                                swap.state_mut().with_binary("body", body);
                                                return Some(swap);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            event!(
                                Level::ERROR,
                                "Unrecognized teleport format {teleport_format}"
                            );
                        }
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
