use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component, BlockObject, BlockProperties, Value};
use tracing::{event, Level};

use crate::proxy::ProxyTarget;

/// Plugin that mirrors image resolution api's, based on OCI spec endpoints -
/// 
/// ```markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-3  | `GET` / `HEAD` | `/v2/<name>/manifests/<reference>`                           | `200`       | `404`             |
/// | end-7  | `PUT`          | `/v2/<name>/manifests/<reference>`                           | `201`       | `404`             |
/// | end-9  | `DELETE`       | `/v2/<name>/manifests/<reference>`                           | `202`       | `404`/`400`/`405` |
/// ```
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Resolve;

impl Plugin for Resolve {
    fn symbol() -> &'static str {
        "resolve"
    }

    fn description() -> &'static str {
        "Resolves an image manifest from the registry. If an artifact_type text attribute exists, will query the referrers api and attach the result"
    }

    fn caveats() -> &'static str {
        "This makes the original call to resolve the image from the desired address, then it passes the response to the mirror proxy implementation"
    }

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async move {
                if let Some(proxy_target) = ProxyTarget::try_from(&tc).ok() {
                    if let Some(manifests) = proxy_target.resolve().await {
                        event!(Level::DEBUG, "{:#?}", manifests);
                    
                        manifests.copy_to_context(&mut tc);
                    }
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }

    // fn compile(parser: &mut lifec::AttributeParser) {
    //     parser.add_custom_with("import", |p, content| {
    //         if let Some(last_entity) = p.last_child_entity() {
    //             p.define_child(last_entity, "import", Value::Symbol(content));
    //         }
    //     })
    // }
}

impl BlockObject for Resolve {
    fn query(&self) -> lifec::BlockProperties {
        BlockProperties::default()
            .require("resolve")
            .require("ns")
            .require("repo")
            .require("reference")
            .require("accept")
            .optional("access_token")
            .optional("digest")
            .optional("protocol")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Resolve::as_custom_attr())
    }
}