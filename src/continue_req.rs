use lifec::{prelude::*, BlockObject, BlockProperties, Plugin};
use poem::{web::headers::Authorization, Request};
use tracing::{event, Level};

/// Plugin that will continue the request from the proxy, using the auth context from the previous state
///
pub struct Continue;

impl Plugin for Continue {
    fn symbol() -> &'static str {
        "continue"
    }

    fn description() -> &'static str {
        "Continues making the request to the upstream server, uses the auth context from the previous plugin state"
    }

    fn caveats() -> &'static str {
        "Useful if all you require is authn or response inspection"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async move {
                if let (Some(ns), Some(api), Some(method), Some(accept), Some(access_token)) = (
                    tc.previous()
                        .expect("previous should exist")
                        .find_symbol("ns"),
                    tc.previous()
                        .expect("previous should exist")
                        .find_symbol("api"),
                    tc.previous()
                        .expect("previous should exist")
                        .find_symbol("method"),
                    tc.previous()
                        .expect("previous should exist")
                        .find_symbol("accept"),
                    tc.previous()
                        .expect("previous should exist")
                        .find_symbol("access_token"),
                ) {
                    let protocol = tc
                        .state()
                        .find_symbol("protocol")
                        .unwrap_or("https".to_string());

                    let url = format!("{protocol}://{ns}/v2/{api}");
                    event!(Level::DEBUG, "Continuing proxied request, {url}");
                    match Authorization::bearer(&access_token) {
                        Ok(auth_header) => {
                            event!(Level::DEBUG, "accept header is: {}", &accept);
                            let req = Request::builder()
                                .uri_str(url.as_str())
                                .typed_header(auth_header.clone())
                                .header("accept", accept)
                                .method(method.parse().expect("should be a valid method"));

                            let req = if let Some(body) = tc
                                .previous()
                                .expect("previous should exist")
                                .find_binary("body")
                            {
                                req.body(body)
                            } else {
                                req.finish()
                            };

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
                                        tc.state_mut().add_symbol(
                                            "digest",
                                            digest.to_str().unwrap_or_default(),
                                        );
                                    }

                                    if let Some(content_type) =
                                        response.headers().get("Content-Type")
                                    {
                                        tc.state_mut().add_symbol(
                                            "content-type",
                                            content_type.to_str().unwrap_or_default(),
                                        );
                                    }

                                    if let Some(location) = response.headers().get("Location") {
                                        tc.state_mut().add_symbol(
                                            "location",
                                            location.to_str().unwrap_or_default(),
                                        );
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

                tc.copy_previous();

                Some(tc)
            }
        })
    }
}

impl BlockObject for Continue {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
            .require("api")
            .require("ns")
            .require("accept")
            .optional("body")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
