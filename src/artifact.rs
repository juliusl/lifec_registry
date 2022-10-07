use crate::{
    content::Descriptor, proxy::ProxyTarget, ArtifactManifest, ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE,
};
use hyper::Method;
use lifec::{AttributeIndex, BlockObject, BlockProperties, Plugin, Value};
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
        "Adds an artifact to a registry"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async {
                // 1) Need to resolve the digest for the subject
                // 2) And then the digest for the blob
                // 3) Luckily these should both be tagged
                if let Some(proxy_target) = ProxyTarget::try_from(&tc).ok() {
                    match (
                        tc.search().find_symbol("subject"),
                        tc.search().find_symbol("blob"),
                    ) {
                        (Some(subject), Some(blob)) => {
                            let accept = tc
                                .search()
                                .find_symbol("accept")
                                .expect("should have accept");

                            let subject_digest = proxy_target
                                .start_request()
                                .expect("should be able to start a request")
                                .uri_str(&subject)
                                .header("accept", &accept)
                                .finish();

                            let blob_digest = proxy_target
                                .start_request()
                                .expect("should be able to start request")
                                .uri_str(blob)
                                .header("accept", &accept)
                                .finish();

                            let subject_desc = proxy_target
                                .send_request(subject_digest)
                                .await
                                .and_then(|resp| {
                                    if resp.status().is_success() {
                                        let digest = resp
                                            .headers()
                                            .get("docker-content-digest")
                                            .expect("should have a digest")
                                            .to_str()
                                            .expect("should be a string");

                                        let content_lengtth = resp
                                            .headers()
                                            .get("content-length")
                                            .expect("should have a content length")
                                            .to_str()
                                            .expect("should be a string")
                                            .parse::<u64>()
                                            .expect("should be an integer");

                                        let content_type = resp
                                            .headers()
                                            .get("content-type")
                                            .expect("should have a content tyype")
                                            .to_str()
                                            .expect("should be a string");

                                        let desc = Descriptor {
                                            media_type: content_type.to_string(),
                                            artifact_type: None,
                                            digest: digest.to_string(),
                                            size: content_lengtth,
                                            annotations: None,
                                            urls: None,
                                            data: None,
                                            platform: None,
                                        };

                                        Some(desc)
                                    } else {
                                        None
                                    }
                                });

                            let blob_desc =
                                proxy_target
                                    .send_request(blob_digest)
                                    .await
                                    .and_then(|resp| {
                                        if resp.status().is_success() {
                                            let digest = resp
                                                .headers()
                                                .get("docker-content-digest")
                                                .expect("should have a digest")
                                                .to_str()
                                                .expect("should be a string");

                                            let content_lengtth = resp
                                                .headers()
                                                .get("content-length")
                                                .expect("should have a content length")
                                                .to_str()
                                                .expect("should be a string")
                                                .parse::<u64>()
                                                .expect("should be an integer");

                                            let content_type = resp
                                                .headers()
                                                .get("content-type")
                                                .expect("should have a content tyype")
                                                .to_str()
                                                .expect("should be a string");

                                            let desc = Descriptor {
                                                media_type: content_type.to_string(),
                                                artifact_type: None,
                                                digest: digest.to_string(),
                                                size: content_lengtth,
                                                annotations: None,
                                                urls: None,
                                                data: None,
                                                platform: None,
                                            };

                                            Some(desc)
                                        } else {
                                            None
                                        }
                                    });

                            let blob_desc = blob_desc.expect("Should be a desc");
                            let subject_desc = subject_desc.expect("should be a desc");

                            let artifact_manifest = ArtifactManifest {
                                media_type: ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE.to_string(),
                                artifact_type: "teleport.link.v1".to_string(),
                                blobs: vec![blob_desc],
                                subject: subject_desc,
                                annotations: None,
                            };

                            event!(Level::DEBUG, "Artifact Manifest\n{:#?}", artifact_manifest);

                            // TODO: check for export 
                            
                            let body = serde_json::to_vec_pretty(&artifact_manifest)
                                .expect("should be serializable");

                            let put = proxy_target.start_request()
                                .expect("should be able to start request")
                                .uri_str(subject)
                                .content_type(&artifact_manifest.media_type)
                                .method(Method::PUT)
                                .body(body);

                            match proxy_target.send_request(put).await {
                                Some(resp) => {
                                    event!(Level::INFO, "Put artifact manifest result {}", resp.status());
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

    fn compile(parser: &mut lifec::AttributeParser) {
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
    fn query(&self) -> lifec::BlockProperties {
        BlockProperties::default()
            .require("artifact")
            .require("subject")
            .optional("blob")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
