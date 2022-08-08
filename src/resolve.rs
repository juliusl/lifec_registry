use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};
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

impl Plugin<ThunkContext> for Resolve {
    fn symbol() -> &'static str {
        "resolve"
    }

    fn description() -> &'static str {
        "Resolves an image manifest from the registry. If an artifact_type text attribute exists, will query the referrers api and attach the result"
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|_| {
            let mut tc = context.clone();
            async move {
                if let (Some(ns), Some(repo), Some(reference), Some(accept), Some(access_token)) = 
                (   tc.as_ref().find_text("ns"), 
                    tc.as_ref().find_text("repo"),
                    tc.as_ref().find_text("reference"),
                    tc.as_ref().find_text("accept"),
                    tc.as_ref().find_text("access_token")
                ) { 

                let manifest_api = format!("https://{ns}/v2/{repo}/manifests/{reference}");
                event!(Level::DEBUG, "Starting image resolution, {manifest_api}");
                match Authorization::bearer(&access_token) {
                    Ok(auth_header) => {
                        event!(Level::DEBUG, "accept header is: {}", &accept);
                        let req = Request::builder()
                            .uri_str(manifest_api.as_str())
                            .typed_header(auth_header.clone())
                            .header("accept", accept)
                            .finish();
                        let client = tc.client().expect("async should be enabled"); 
                        match client.request(req.into()).await {
                            Ok(response) => {
                                if let Some(digest) = response.headers().get("Docker-Content-Digest") {
                                    debug_assert!(!digest.is_sensitive(), "docker-content-digest should not be a sensitive header");
                                    event!(Level::DEBUG, "Resolved digest is {:?}", digest);
                                    tc.as_mut().add_text_attr(
                                        "digest", 
                                        digest.to_str().unwrap_or_default()
                                    );
                                }

                                if let Some(content_type) = response.headers().get("Content-Type") {
                                    debug_assert!(!content_type.is_sensitive(), "content-type should not be a sensitive header");
                                    event!(Level::DEBUG, "Resolved content-type is {:?}", content_type);
                                    tc.as_mut().add_text_attr(
                                        "content-type", 
                                        content_type.to_str().expect("Content-Type must be a valid string")
                                    );
                                }

                                match hyper::body::to_bytes(response.into_body()).await {
                                    Ok(data) => {
                                        event!(Level::DEBUG, "Resolved manifest, len: {}", data.len());
                                        event!(Level::TRACE, "{:#?}", data);

                                        tc.as_mut().add_binary_attr("body", data);
                                    },
                                    Err(err) =>  event!(Level::ERROR, "Could not read response body, {err}")
                                }

                                // In order to call the referrer's api, we must have an artifact_type and digest to filter the
                                // the response
                                // TODO: It would be nice to support multiple artifact_types such as
                                // define obd      artifact_type dadi.v1
                                // define teleport artifact_type teleport.v1
                                // And then the transient value is the response from the referrer's api
                                // 
                                if let (Some(artifact_type), Some(digest)) = (
                                    tc.as_ref().find_text("artifact_type"), 
                                    tc.as_ref().find_text("digest")
                                ) {
                                    event!(Level::DEBUG, "Making referrers call for {artifact_type}");
                                    let referrers_api = format!("https://{ns}/v2/{repo}/_oras/artifacts/referrers?digest={digest}&artifactType={artifact_type}");
                                    let req = Request::builder()
                                        .uri_str(referrers_api.as_str())
                                        .typed_header(auth_header)
                                        .finish();
        
                                    match client.request(req.into()).await {
                                        Ok(response) => { 
                                            event!(Level::TRACE, "{:#?}", response);
                                            match hyper::body::to_bytes(response.into_body()).await {
                                                Ok(data) => tc.as_mut().add_binary_attr("referrers", data),
                                                Err(err) =>  event!(Level::ERROR, "Could not read referrers response body {err}")
                                            }
                                        }
                                        Err(err) => event!(Level::ERROR, "Could not send request for referrers api, {err}")
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
