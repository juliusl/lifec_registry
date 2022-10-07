use std::str::from_utf8;

use lifec::BlockObject;
use poem::{Request, web::headers::Authorization};
use lifec::Plugin;

use lifec::AttributeIndex;
use tracing::event;
use tracing::Level;

/// Plugin for calling the referrer's api and adding the result to state,
/// 
#[derive(Default)]
pub struct Discover;

impl Plugin for Discover {
    fn symbol() -> &'static str {
        "discover"
    }

    fn description() -> &'static str {
        "Uses the registry referrer's api to find artifacts by type and subject digest"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async move {
                if let (Some(ns), Some(repo), Some(artifact_type), Some(digest), Some(access_token)) = 
                (   tc.search().find_symbol("ns"), 
                    tc.search().find_symbol("repo"),
                    tc.state().find_symbol("discover"),
                    tc.search().find_symbol("digest"),
                    // Check previous state for access token
                    tc.search().find_symbol("access_token")
                ) { 

                event!(Level::DEBUG, "Discovering {artifact_type}");

                let protocol = tc.search()
                    .find_symbol("protocol")
                    .unwrap_or("https".to_string());

                match Authorization::bearer(&access_token) {
                    Ok(auth_header) => {
                        let client = tc.client().expect("async should be enabled"); 
                        let api = tc.state()
                            .find_symbol("referrers_api")
                            .unwrap_or("_oras/artifacts/referrers".to_string());

                        let referrers_api = format!("{protocol}://{ns}/v2/{repo}/{api}?digest={digest}&artifactType={artifact_type}");
                        event!(Level::DEBUG, "Making referrers call for {artifact_type}\n{referrers_api}");
                        let req = Request::builder()
                            .uri_str(referrers_api.as_str())
                            .typed_header(auth_header)
                            .finish();

                        match client.request(req.into()).await {
                            Ok(response) => { 
                                match hyper::body::to_bytes(response.into_body()).await {
                                    Ok(data) => { 
                                        event!(Level::TRACE, "{:#?}", from_utf8(&data).ok());
                                        tc.state_mut().add_binary_attr(
                                        artifact_type, 
                                        data
                                    )},
                                    Err(err) =>  event!(Level::ERROR, "Could not read referrers response body {err}")
                                }
                            }
                            Err(err) => event!(Level::ERROR, "Could not send request for referrers api, {err}")
                        }
                    }
                    Err(err) => event!(Level::ERROR, "Could not create auth bearer header, {err}")
                }}

                tc.copy_previous();
                Some(tc)
            }
        })
    }
}

impl BlockObject for Discover {
    fn query(&self) -> lifec::BlockProperties {
        lifec::BlockProperties::default()
            .require("discover")
            .require("digest")
            .require("repo")
            .require("ns")
            .require("access_token")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

