use crate::{
    ProxyTarget, ArtifactManifest, ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE,
};
use hyper::Method;
use lifec::prelude::{AttributeIndex, BlockObject, BlockProperties, Plugin, Value, AsyncContext, ThunkContext, AttributeParser, CustomAttribute};
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

    fn call(context: &ThunkContext) -> Option<AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async {
                if let Some(proxy_target) = ProxyTarget::try_from(&tc).ok() {
                    match (
                        tc.search().find_symbol("subject"),
                        tc.search().find_symbol("blob"),
                    ) {
                        (Some(subject), Some(blob)) => {
                            let subject_desc = proxy_target.resolve_descriptor(&subject).await;
                            let subject_desc = subject_desc.expect("should be a desc");

                            // TODO - handle list of blobs
                            let blob_desc = proxy_target.resolve_descriptor(&blob).await;
                            let blob_desc = blob_desc.expect("Should be a desc");

                            let artifact_manifest = ArtifactManifest {
                                media_type: ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE.to_string(),
                                artifact_type: "teleport.link.v1".to_string(),
                                blobs: vec![
                                    blob_desc
                                ],
                                subject: subject_desc,
                                annotations: None,
                            };

                            event!(Level::DEBUG, "Artifact Manifest\n{:#?}", artifact_manifest);

                            let body = serde_json::to_vec_pretty(&artifact_manifest)
                                .expect("should be serializable");
                            
                            let artifact_uri = format!("{}-link", proxy_target.manifest_url());

                            let put = proxy_target.start_request()
                                .expect("should be able to start request")
                                .uri_str(&artifact_uri)
                                .content_type(&artifact_manifest.media_type)
                                .method(Method::PUT)
                                .body(body);

                            match proxy_target.send_request(put).await {
                                Some(resp) => {
                                    if !resp.status().is_success() {
                                        event!(Level::ERROR, "Could not put manifest {}", artifact_uri);
                                    } else {
                                        event!(Level::INFO, "Put artifact manifest result {}", resp.status());
                                    }
                                },
                                None => {
                                    event!(Level::ERROR, "Could not put manifest");
                                    
                                },
                            }
                        }
                        (None, None) => event!(Level::ERROR, "Missing subject and blob"),
                        (None, Some(_)) => event!(Level::ERROR, "Missing subject"),
                        (Some(_), None) => event!(Level::ERROR, "Missing blob"),
                    }
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }

    fn compile(parser: &mut AttributeParser) {
        parser.add_custom_with("subject", |p, content| {
            if let Some(last) = p.last_child_entity() {
                p.define_child(last, "subject", Value::Symbol(content));
            }
        });

        parser.add_custom_with("blob", |p, content| {
            if let Some(last) = p.last_child_entity() {
                p.define_child(last, "blob", Value::Symbol(content));
            }
        });
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
