use hyper::Method;
use lifec::{prelude::*, BlockObject, BlockProperties, CustomAttribute, Plugin, Value};
use poem::{web::headers::Authorization, Request};
use tracing::{event, Level};

/// Plugin that will continue the request from the proxy, using the auth context from the previous state
///
#[derive(Default)]
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
                if let (Some(api), Some(method), Some(accept), Some(access_token)) = (
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
                    let url = format!("{api}");
                    event!(Level::DEBUG, "Continuing proxied request, {url}");
                    match Authorization::bearer(&access_token) {
                        Ok(auth_header) => {
                            let req = Request::builder()
                                .uri_str(url.as_str())
                                .typed_header(auth_header.clone())
                                .header("accept", {
                                    if let Some(accept) = tc.state().find_symbol("accept") {
                                        event!(Level::DEBUG, "accept header is: {}", &accept);
                                        accept
                                    } else {
                                        event!(Level::DEBUG, "accept header is: {}", &accept);
                                        accept
                                    }
                                })
                                .method(
                                    Method::from_bytes(method.to_ascii_uppercase().as_bytes())
                                        .unwrap(),
                                );

                            let req = if let Some(body) = tc
                                .previous()
                                .expect("previous should exist")
                                .find_binary("body")
                            {
                                event!(Level::DEBUG, "Attaching body to request");
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
                                    } else if let Some(digest) = tc
                                        .previous()
                                        .expect("should have a previous state")
                                        .find_symbol("digest")
                                    {
                                        event!(
                                            Level::DEBUG,
                                            "Resolved digest from state {}",
                                            &digest
                                        );
                                        tc.state_mut().add_symbol("digest", digest);
                                    }

                                    if let Some(content_type) =
                                        response.headers().get("Content-Type")
                                    {
                                        event!(
                                            Level::DEBUG,
                                            "Resolved content-type {:?}",
                                            &content_type.to_str()
                                        );
                                        tc.state_mut().add_symbol(
                                            "content-type",
                                            content_type.to_str().unwrap_or_default(),
                                        );
                                    }

                                    if let Some(location) = response.headers().get("Location") {
                                        event!(
                                            Level::DEBUG,
                                            "Resolved location {:?}",
                                            &location.to_str()
                                        );
                                        tc.state_mut().add_symbol(
                                            "location",
                                            location.to_str().unwrap_or_default(),
                                        );
                                    };

                                    event!(
                                        Level::DEBUG,
                                        "Resolved status code {}",
                                        response.status().as_str()
                                    );
                                    tc.state_mut()
                                        .add_symbol("status_code", response.status().as_str());

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

    fn compile(parser: &mut lifec::AttributeParser) {
        parser.add_custom(CustomAttribute::new_with("accept", |p, content| {
            if let Some(last_entity) = p.last_child_entity() {
                p.define_child(last_entity, "accept", Value::Symbol(content));
            }
        }));
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
