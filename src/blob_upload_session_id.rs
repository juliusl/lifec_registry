use hyper::Method;
use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};
use poem::{web::headers::Authorization, Request};
use tracing::{event, Level};


/// Retrieves a blob upload session id from the registry
/// 
/// 
/// ``` markdown
/// | ID     | Method         | API Endpoint                                                 | Success     | Failure           |
/// | ------ | -------------- | ------------------------------------------------------------ | ----------- | ----------------- |
/// | end-4a | `POST`         | `/v2/<name>/blobs/uploads/`                                  | `202`       | `404`             |
/// ```
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct BlobUploadSessionId; 

impl Plugin<ThunkContext> for BlobUploadSessionId {
    fn symbol() -> &'static str {
        "blob_upload_session_id"
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|_| {
            let mut tc = context.clone();
            async move {
                if let (Some(ns), Some(name), Some(access_token)) = 
                (   tc.as_ref().find_text("ns"), 
                    tc.as_ref().find_text("name"),
                    tc.as_ref().find_text("access_token")
                ) {
                    let upload_session_id = format!("https://{ns}/v2/{name}/blobs/uploads");
                    event!(Level::DEBUG, "Starting blob upload, {upload_session_id}");
                    match Authorization::bearer(&access_token) {
                        Ok(auth_header) => {
                            let req = Request::builder()
                                .uri_str(upload_session_id.as_str())
                                .typed_header(auth_header.clone())
                                .method(Method::POST)
                                .finish();
                            let client = tc.client().expect("async should be enabled");
                             
                            match client.request(req.into()).await {
                                Ok(resp) => {
                                    if let Some(location) = resp.headers().get("Location") {
                                        match location.to_str() {
                                            Ok(location) => {
                                                tc.as_mut().add_text_attr("location", location);

                                                return Some(tc);
                                            },
                                            Err(err) => {
                                                event!(Level::ERROR, "error getting location header, {err}");
                                            },
                                        }
                                    }
                                },
                                Err(err) => {
                                    event!(Level::ERROR, "error sending request, {err}")
                                },
                            }
                        }
                        Err(err) => {
                            event!(Level::ERROR, "error getting auth header, {err}")
                        },
                    }
                }

                None
            }
        })
    }
}