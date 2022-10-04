use hyper::Method;
use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component, AttributeIndex};
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
pub struct PushSession; 

impl Plugin for PushSession {
    fn symbol() -> &'static str {
        "push_session"
    }

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|_| {
            let mut tc = context.clone();
            async move {
                if let (Some(ns), Some(name), Some(access_token)) = 
                (   tc.previous().expect("should be a previous state").find_symbol("ns"), 
                    tc.previous().expect("should be a previous state").find_symbol("name"),
                    tc.previous().expect("should be a previous state").find_symbol("access_token")
                ) {
                    let protocol = tc.previous()
                        .expect("should be a previous state")
                        .find_symbol("protocol")
                        .unwrap_or("https".to_string());

                    let upload_session_id = format!("{protocol}://{ns}/v2/{name}/blobs/uploads");
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
                                                tc.state_mut().add_text_attr("location", location);

                                                tc.copy_previous();
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