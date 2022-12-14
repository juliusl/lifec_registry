use crate::{ArtifactManifest, ProxyTarget, ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE, OCI_ARTIFACTS_MANIFEST_MEDIA_TYPE};
use hyper::Method;
use lifec::prelude::{
    AddDoc, AsyncContext, AttributeIndex, AttributeParser, BlockObject, BlockProperties,
    CustomAttribute, Plugin, ThunkContext, Value,
};
use tracing::{event, Level};

/// This plugin is for adding artifacts to a registry,
///
#[derive(Default)]
pub struct Artifact;

impl Plugin for Artifact {
    fn symbol() -> &'static str {
        "artifact"
    }

    fn description() -> &'static str {
        "Defines and adds an artifact manifest to the registry"
    }

    fn call(context: &mut ThunkContext) -> Option<AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async {
                if let Some(proxy_target) = ProxyTarget::try_from(&tc).ok() {
                    match (
                        tc.search().find_symbol("artifact"),
                        tc.search().find_symbol("subject"),
                        tc.search().find_symbol_values("blob"),
                    ) {
                        (Some(artifact_type), Some(subject), blob_vec) => {
                            let subject_desc = proxy_target.resolve_descriptor(&subject).await;
                            let subject_desc = subject_desc.expect("should be a desc");

                            let mut blobs = vec![];
                            if let Some(blob) = blob_vec.first() {
                                // TODO - handle list of blobs
                                let blob_desc = proxy_target.resolve_descriptor(blob).await;
                                let blob_desc = blob_desc.expect("Should be a desc");
                                blobs.push(blob_desc);
                            }

                            let artifact_manifest = ArtifactManifest {
                                media_type: if tc.is_enabled("oci") {
                                    OCI_ARTIFACTS_MANIFEST_MEDIA_TYPE.into()
                                } else {
                                    ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE.into()
                                },
                                artifact_type,
                                blobs,
                                subject: subject_desc,
                                annotations: None,
                            };

                            event!(Level::DEBUG, "Artifact Manifest\n{:#?}", artifact_manifest);

                            let body = serde_json::to_vec_pretty(&artifact_manifest)
                                .expect("should be serializable");

                            let artifact_uri = format!("{}-link", proxy_target.manifest_url());

                            let put = proxy_target
                                .start_request()
                                .uri_str(&artifact_uri)
                                .content_type(&artifact_manifest.media_type)
                                .method(Method::PUT)
                                .body(body);

                            match proxy_target.send_request(put).await {
                                Some(resp) => {
                                    if !resp.status().is_success() {
                                        event!(
                                            Level::ERROR,
                                            "Could not put manifest {}, {:?}",
                                            artifact_uri,
                                            resp
                                        );
                                    } else {
                                        event!(
                                            Level::INFO,
                                            "Put artifact manifest result {}",
                                            resp.status()
                                        );
                                    }
                                }
                                None => {
                                    event!(Level::ERROR, "Could not put manifest");
                                }
                            }
                        }
                        (None, _, _) => event!(Level::ERROR, "Missing artifact type"),
                        (_, None, _) => event!(Level::ERROR, "Missing subject"),
                    }
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }

    fn compile(parser: &mut AttributeParser) {
        if let Some(mut docs) = Self::start_docs(parser) {
            let docs = &mut docs;
            docs.as_mut().add_custom_with("subject", |p, content| {
                if let Some(last) = p.last_child_entity() {
                    p.define_child(last, "subject", Value::Symbol(content));
                }
            })
            .add_doc(docs, "The subject of this artifact")
            .symbol("This should be an image reference uri to the subject. It will be resolved into a descriptor.");

            docs.as_mut().add_custom_with("blob", |p, content| {
                if let Some(last) = p.last_child_entity() {
                    p.define_child(last, "blob", Value::Symbol(content));
                }
            })
            .add_doc(docs, "A blob of this artifact")
            .list()
            .symbol("This should be an image reference uri to the blob. It will be resolved into a descriptor.");

            docs.as_mut().add_custom_with("oci", |p, _| {
                if let Some(last) = p.last_child_entity() {
                    p.define_child(last, "oci", true);
                }
            })
            .add_doc(docs, "Uses the OCI artifact media type instead of ORAS artifact media type");
        }
    }
}

impl BlockObject for Artifact {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
            .require("artifact")
            .require("subject")
            .optional("blob")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
