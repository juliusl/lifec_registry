use hyper::Method;
use lifec::{AttributeIndex, BlockObject, BlockProperties, Plugin, ThunkContext};
use poem::{web::headers::Authorization, Request};
use tracing::{event, Level};

/// Plugin to upload registry content
///
#[derive(Default)]
pub struct Upload;

impl Plugin for Upload {
    fn symbol() -> &'static str {
        "upload"
    }

    fn description() -> &'static str {
        "Uploads content to the registry"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async {
                let method = tc
                    .search()
                    .find_symbol("method")
                    .expect("Should have been added by the proxy handler");

                match method.as_str() {
                    "post" => {
                        
                    },
                    "put" => {

                    },
                    "patch" => {

                    },
                    _ => {

                    }
                }

                Some(tc)
            }
        })
    }
}

impl Upload {
    /// Upload using registry's session id method,
    ///
    pub async fn upload_session_id(tc: &ThunkContext) -> Option<ThunkContext> {
        let mut tc = tc.clone();

        if let (Some(ns), Some(name), Some(access_token)) = (
            tc.search().find_symbol("ns"),
            tc.search().find_symbol("name"),
            tc.search().find_symbol("access_token"),
        ) {
            let protocol = tc
                .previous()
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
                                    }
                                    Err(err) => {
                                        event!(
                                            Level::ERROR,
                                            "error getting location header, {err}"
                                        );
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            event!(Level::ERROR, "error sending request, {err}")
                        }
                    }
                }
                Err(err) => {
                    event!(Level::ERROR, "error getting auth header, {err}")
                }
            }
        }

        None
    }
}

impl BlockObject for Upload {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
