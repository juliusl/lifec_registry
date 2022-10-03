use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component, AttributeIndex, BlockObject, BlockProperties};
use poem::{Request, web::headers::Authorization};
use tracing::{event, Level};

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
                if let (Some(ns), Some(repo), Some(reference), Some(accept), Some(access_token)) = 
                (   tc.previous().unwrap().find_symbol("ns"), 
                    tc.previous().unwrap().find_symbol("repo"),
                    tc.previous().unwrap().find_symbol("reference"),
                    tc.previous().unwrap().find_symbol("accept"),
                    // Check previous state for access token
                    tc.previous().unwrap().find_symbol("access_token")
                ) { 

                let protocol = tc.state()
                    .find_symbol("protocol")
                    .unwrap_or("https".to_string());
                
                let manifest_api = format!("{protocol}://{ns}/v2/{repo}/manifests/{reference}");
                
                event!(Level::DEBUG, "Starting image resolution, {manifest_api}");
                match Authorization::bearer(&access_token) {
                    Ok(auth_header) => {
                        event!(Level::DEBUG, "Accept header is: {}", &accept);
                        let req = Request::builder()
                            .uri_str(manifest_api.as_str())
                            .typed_header(auth_header.clone())
                            .header("accept", {
                                if let Some(resolve) = tc.state().find_symbol("resolve") {
                                    event!(Level::DEBUG, "Setting accept to {resolve}");
                                    resolve
                                } else {
                                    accept
                                }
                            })
                            .finish();
                        let client = tc.client().expect("async should be enabled"); 
                        match client.request(req.into()).await {
                            Ok(response) => {
                                if let Some(digest) = response.headers().get("Docker-Content-Digest") {
                                    debug_assert!(!digest.is_sensitive(), "docker-content-digest should not be a sensitive header");
                                    event!(Level::DEBUG, "Resolved digest is {:?}", digest);
                                    tc.state_mut().add_symbol(
                                        "digest", 
                                        digest.to_str().unwrap_or_default()
                                    );
                                }

                                if let Some(content_type) = response.headers().get("Content-Type") {
                                    debug_assert!(!content_type.is_sensitive(), "content-type should not be a sensitive header");
                                    event!(Level::DEBUG, "Resolved content-type is {:?}", content_type);
                                    tc.state_mut().add_symbol(
                                        "content-type", 
                                        content_type.to_str().expect("Content-Type must be a valid string")
                                    );
                                }

                                match hyper::body::to_bytes(response.into_body()).await {
                                    Ok(data) => {
                                        event!(Level::DEBUG, "Resolved manifest, len: {}", data.len());
                                        event!(Level::TRACE, "{:#?}", data);

                                        tc.state_mut().add_binary_attr("body", data);
                                    },
                                    Err(err) =>  event!(Level::ERROR, "Could not read response body, {err}")
                                }

                                for (name, value) in tc.previous().expect("Should have been a previous state").values() {
                                    for value in value {
                                        tc.state_mut().with(&name, value);
                                    }
                                }

                                event!(Level::INFO, "Mirrored resolve registry resolve api");
                                return Some(tc);
                            },
                            Err(err) => event!(Level::ERROR, "Could not resolve image manifest, {err}")
                        }
                    }
                    Err(err) => event!(Level::ERROR, "Could not create auth bearer header, {err}")
                }}

                event!(Level::WARN, "Could not mirror resolve api");
                None
            }
        })
    }
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