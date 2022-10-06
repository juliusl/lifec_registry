use lifec::{Plugin, BlockObject, BlockProperties, CustomAttribute, Value};
use logos::{Logos, Lexer};
use serde_json::json;
use crate::{proxy::ProxyTarget, content::{OverlaybdArtifact, Descriptor}};


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
                if let Some(proxy_target) = ProxyTarget::try_from(&tc).ok() {
                    let subject = json!({
                        "digest":"sha256:ed8cba11c09451dbb3495f15951e4afb4f1ba72a4a13e135c6da06c6346e0333",
                        "mediaType":"application/vnd.docker.distribution.manifest.list.v2+json",
                        "size":1862
                    });
                    let subject = serde_json::from_value::<Descriptor>(subject).expect("should be a descriptor");
                
                    let artifact = json!({
                        "digest":"sha256:20ac9fbb5ae6000b78f7e825bef3b7f01710048d939fe6571248b435b01ff8ba",
                        "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
                        "size": 3363
                    });
                    let artifact = serde_json::from_value::<Descriptor>(artifact).expect("should be a descriptor");

                    let overlaybd = OverlaybdArtifact::new(subject, artifact);
                
                    overlaybd.artifact().upload(&tc).await;
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }

    /// # Example usage
    /// 
    /// ```md
    /// .process sh convert.sh
    /// 
    /// .artifact artifact.example.v1
    /// - This will get resolved when the plugin is called
    /// 
    /// .subject {registry-name}.{registry-host}/{repo}:{reference}
    /// .blob    world://{subject}/sbom.json, application/json
    /// .blob    world://{subject}/output.txt, text/utf8
    /// .blob    registry://{registry-name}.{registry-host}/{repo}:{reference}-obd, application/vnd.oci.image.manifest.v1+json
    /// 
    /// ```
    /// 
    fn compile(parser: &mut lifec::AttributeParser) {
        parser.add_custom(CustomAttribute::new_with("subject", |p, content| {
            if let Some(last) = p.last_child_entity() {
                p.define_child(last, "subject", Value::Symbol(content));
            }
        }));

        parser.add_custom(CustomAttribute::new_with("blob", |p, content| {
            
        }));
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

/// Enumeration of blob tokens to help parse a .blob attr
/// 
#[derive(Logos)]
enum BlobTokens {
    /// File that will exist in the world_dir
    /// 
    #[token("world://", on_world_file)]
    WorldFile(String),
    /// Image reference that must be resolved
    /// 
    #[token("registry://", on_image_reference)]
    ImageReference(String),
    /// Media type
    /// 
    #[token("application/", on_media_type)]
    MediaType(String),
    #[error]
    #[regex(r"[ ,\t\n\f]+", logos::skip)]
    Error,
}

fn on_world_file(lexer: &mut Lexer<BlobTokens>) -> Option<String> { 
    None 
}

fn on_image_reference(lexer: &mut Lexer<BlobTokens>) -> Option<String> { 
    None 
}

fn on_media_type(lexer: &mut Lexer<BlobTokens>) -> Option<String> { 
    None 
}