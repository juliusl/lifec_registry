use lifec::{
    plugins::{Plugin, ThunkContext},
    AttributeIndex, Component, DenseVecStorage,
};
use poem::{web::headers::Authorization, Request};
use tracing::{event, Level};

/// Blob download handler based on OCI spec endpoints:
///
/// ```markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-2  | `GET` / `HEAD` | `/v2/<name>/blobs/<digest>`                                  | `200`       | `404`             |
/// | end-10 | `DELETE`       | `/v2/<name>/blobs/<digest>`                                  | `202`       | `404`/`405`       |
/// ```
///
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct DownloadBlob;

impl Plugin for DownloadBlob {
    fn symbol() -> &'static str {
        "download_blob"
    }

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|_| {
            let mut tc = context.clone();
            async move {
                if let (Some(ns), Some(name), Some(digest), Some(accept), Some(access_token)) = (
                    tc.state().find_symbol("ns"),
                    tc.state().find_symbol("name"),
                    tc.state().find_symbol("digest"),
                    tc.state().find_symbol("accept"),
                    tc.previous().and_then(|p| p.find_symbol("access_token")),
                ) {
                    let protocol = tc
                        .state()
                        .find_symbol("protocol")
                        .unwrap_or("https".to_string());

                    let download_api = format!("{protocol}://{ns}/v2/{name}/blobs/{digest}");
                    event!(Level::DEBUG, "Starting blob download, {download_api}");
                    match Authorization::bearer(&access_token) {
                        Ok(auth_header) => {
                            event!(Level::DEBUG, "accept header is: {}", &accept);
                            let req = Request::builder()
                                .uri_str(download_api.as_str())
                                .typed_header(auth_header.clone())
                                .header("accept", accept)
                                .finish();
                            let client = tc.client().expect("async should be enabled");
                            match client.request(req.into()).await {
                                Ok(response) => {
                                    event!(
                                        Level::TRACE,
                                        "Received response for blob download, {:#?}",
                                        response
                                    );

                                    if let Some(digest) =
                                        response.headers().get("Docker-Content-Digest")
                                    {
                                        event!(
                                            Level::DEBUG,
                                            "Resolved digest is {:?}",
                                            &digest.to_str()
                                        );
                                        tc.state_mut().add_text_attr(
                                            "digest",
                                            digest.to_str().unwrap_or_default(),
                                        );
                                    }

                                    if let Some(content_type) =
                                        response.headers().get("Content-Type")
                                    {
                                        tc.state_mut().add_text_attr(
                                            "content-type",
                                            content_type.to_str().unwrap_or_default(),
                                        );
                                    }

                                    let response = if let Some(location) =
                                        response.headers().get("Location")
                                    {
                                        client
                                            .get(
                                                location
                                                    .to_str()
                                                    .unwrap_or_default()
                                                    .parse()
                                                    .unwrap(),
                                            )
                                            .await
                                            .unwrap()
                                    } else {
                                        response
                                    };

                                    match hyper::body::to_bytes(response.into_body()).await {
                                        Ok(data) => {
                                            event!(
                                                Level::DEBUG,
                                                "Resolved blob, len: {}",
                                                data.len()
                                            );
                                            event!(Level::TRACE, "{:#?}", data);

                                            tc.state_mut().add_binary_attr("body", data);
                                        }
                                        Err(err) => event!(Level::ERROR, "{err}"),
                                    }

                                    return Some(tc);
                                }
                                Err(err) => event!(Level::ERROR, "{err}"),
                            }
                        }
                        Err(err) => event!(Level::ERROR, "{err}"),
                    }
                }

                None
            }
        })
    }
}
