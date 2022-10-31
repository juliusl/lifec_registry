use lifec::prelude::{
    AttributeIndex, BlockObject, BlockProperties, Component, CustomAttribute, DenseVecStorage,
    Plugin, ThunkContext,
};
use tracing::{event, Level};

/// Plugin that mirrors image resolution api's, based on OCI spec endpoints,
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
        "Resolves a digest from a cached response and saves it to state"
    }

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        let digest = context.cached_response().and_then(|c| c.headers().get("docker-content-digest")).cloned();
        
        context.task(|_| {
            let mut tc = context.clone();
            async move {
                if let Some(digest) = digest {
                    event!(Level::DEBUG, "Found digest {:?}", digest); 
                    tc.state_mut().with_symbol("digest", digest.to_str().expect("should be a string"));
                } else {
                    event!(Level::ERROR, "Did not find digest from cached response"); 
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }
}

impl BlockObject for Resolve {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Resolve::as_custom_attr())
    }
}
