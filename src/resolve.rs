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
                        let manifest_accept = tc.as_ref().find_text("manifest_accept").unwrap_or("application/vnd.docker.distribution.manifest.list.v2+json".to_string());
                        let req = Request::builder()
                            .uri_str(manifest_api.as_str())
                            .typed_header(auth_header.clone())
                            .header("accept", manifest_accept)
                            .finish();
                        let client = tc.client().expect("async should be enabled"); 
                        match client.request(req.into()).await {
                            Ok(response) => {                
                                event!(Level::TRACE, "Received response for manifest call, {:#?}", response);

                                if let Some(digest) = response.headers().get("Docker-Content-Digest") {                                
                                    event!(Level::DEBUG, "Resolved digest is {:?}", &digest.to_str());
                                    tc.as_mut().add_text_attr(
                                        "digest", 
                                        digest.to_str().unwrap_or_default()
                                    );
                                }

                                if let Some(content_type) = response.headers().get("Content-Type") {
                                    tc.as_mut().add_text_attr(
                                        "content-type", 
                                        content_type.to_str().unwrap_or("application/vnd.docker.distribution.manifest.list.v2+json")
                                    );
                                }

                                match hyper::body::to_bytes(response.into_body()).await {
                                    Ok(data) => {
                                        event!(Level::DEBUG, "Resolved manifest, len: {}", data.len());
                                        event!(Level::TRACE, "{:#?}", data);

                                        tc.as_mut().add_binary_attr("body", data);
                                    },
                                    Err(err) =>  event!(Level::ERROR, "{err}")
                                }

                                if let (Some(artifact_type), Some(digest)) = (
                                    tc.as_ref().find_text("artifact_type"), 
                                    tc.as_ref().find_text("digest")
                                ) {
                                    let referrers_api = format!("https://{ns}/v2/{repo}/_oras/artifacts/referrers?digest={digest}&artifactType={artifact_type}");
                                    let req = Request::builder()
                                        .uri_str(referrers_api.as_str())
                                        .typed_header(auth_header)
                                        .finish();
        
                                    match client.request(req.into()).await {
                                        Ok(response) => match hyper::body::to_bytes(response.into_body()).await {
                                            Ok(data) => tc.as_mut().add_binary_attr("referrers", data),
                                            Err(err) =>  event!(Level::ERROR, "{err}")
                                        }
                                        Err(err) => event!(Level::ERROR, "{err}")
                                    }
                                }

                                return Some(tc);
                            },
                            Err(err) => event!(Level::ERROR, "{err}")
                        }
                    }
                    Err(err) => event!(Level::ERROR, "{err}")
                }}

                None
            }
        })
    }
}
