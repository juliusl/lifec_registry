use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};
use poem::{Request, web::headers::Authorization};
use tracing::{event, Level};

/// BlobImport handler based on OCI spec endpoints: 
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
                match Authorization::bearer(&access_token) {
                    Ok(auth_header) => {
                        let req = Request::builder()
                            .uri_str(manifest_api.as_str())
                            .typed_header(auth_header.clone())
                            .header("accept", accept)
                            .finish();
                        let client = tc.client().expect("async should be enabled"); 
                        match client.request(req.into()).await {
                            Ok(response) => {
                                if let Some(digest) = response.headers().get("Docker-Content-Digest") {
                                    tc.as_mut().add_text_attr("digest", digest.to_str().unwrap_or_default());
                                }
                                match hyper::body::to_bytes(response.into_body()).await {
                                    Ok(data) => tc.as_mut().add_binary_attr("manifest", data),
                                    Err(err) =>  event!(Level::ERROR, "{err}")
                                }
                            },
                            Err(err) => event!(Level::ERROR, "{err}")
                        }
                        
                        if let Some(artifact_type) = tc.as_ref().find_text("artifact_type") {
                            let referrers_api = format!("https://{ns}/v2/{repo}/_oras/artifacts/referrers?artifactType={artifact_type}");
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
                    }
                    Err(err) => event!(Level::ERROR, "{err}")
                }}
                None 
            }
        })
    }
}
