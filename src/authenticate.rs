use hyper::{http, Uri, Method};
use lifec::{
    plugins::{Plugin, ThunkContext},
    Component, DenseVecStorage,
};
use poem::{web::headers::Authorization, Request};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{event, Level};

/// Plugin for authenticating w/ a registry
///
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Authenticate;

#[derive(Deserialize, Serialize)]
pub struct Credentials {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "refreshToken")]
    refresh_token: Option<String>,
}

impl Authenticate {
    /// Gets the challenge header from the registry
    ///
    async fn start_challenge(tc: &ThunkContext) -> Option<Uri> {
        if let Some(client) = tc.client() {
            let api = tc
                .as_ref()
                .find_text("api")
                .and_then(|a| Uri::from_str(a.as_str()).ok());

            event!(
                Level::TRACE,
                "{:?}, {:?}",
                tc.as_ref().find_text("api"),
                api
            );
            if let Some(api) = api {
                event!(Level::DEBUG, "calling {api} to initiate authn");
                if let Some(response) = client.get(api.clone()).await.ok() {
                    if response.status().is_client_error() {
                        event!(
                            Level::DEBUG,
                            "client error detected, starting auth challenge"
                        );
                        event!(Level::TRACE, "{:#?}", response);
                        let challenge = response
                            .headers()
                            .get(http::header::WWW_AUTHENTICATE)
                            .expect("401 should've been returned w/ a challenge header");
                        let challenge = challenge
                            .to_str()
                            .expect("challenge header should be a string");
                        let challenge = Self::parse_challenge_header(challenge);

                        event!(Level::DEBUG, "received challange {challenge}");

                        return Some(
                            Uri::from_str(&challenge).expect("challenge should be a valid uri"),
                        );
                    }
                }
            }
        }

        event!(Level::WARN, "did not authn request, exiting");
        None
    }

    /// Authenticates the request to the registry and returns credentials
    ///
    async fn authenticate(tc: &ThunkContext) -> Option<Credentials> {
        if let Some(challenge_uri) = Self::start_challenge(tc).await {
            if let (Some(ns), Some(token)) =
                (tc.as_ref().find_text("ns"), tc.as_ref().find_text("token"))
            {
                event!(Level::DEBUG, "Begining authn for {challenge_uri}");
                
                // curl -v -X POST -H "Content-Type: application/x-www-form-urlencoded" -d \
                // "grant_type=refresh_token&service=$registry&scope=$scope&refresh_token=$acr_refresh_token" \
                // https://$registry/oauth2/token
                
                let challenge_uri = format!("{}&grant_type=refresh_token&refresh_token={}", challenge_uri.to_string(), token).parse::<Uri>().expect("should be valid");

                let req = Request::builder()
                    .uri(challenge_uri)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .method(Method::POST)
                    .finish();

                let client = tc
                    .client()
                    .expect("async is enabled, so this should be set");
                
                event!(Level::TRACE, "{:#?}", req);
                match client.request(req.into()).await {
                    Ok(response) => {
                        event!(Level::TRACE, "{:#?}", response);
                        match hyper::body::to_bytes(response.into_body()).await {
                            Ok(bytes) => {
                                return serde_json::de::from_slice::<Credentials>(bytes.as_ref()).ok()
                            }
                            Err(err) => {
                                event!(Level::ERROR, "Could not decode credentials, {ns} {err}")
                            }
                        }
                    },
                    Err(err) => event!(Level::ERROR, "Could not fetch credentials for, {ns} {err}"),
                }
            }
        }

        None
    }

    fn parse_challenge_header(challenge: impl AsRef<str>) -> String {
        challenge
            .as_ref()
            .trim_start_matches(r#"Bearer realm=""#)
            .replace(r#"",service="#, r#"?service="#)
            .replace(",", "&")
            .replace('"', "")
    }
}

impl Plugin<ThunkContext> for Authenticate {
    fn symbol() -> &'static str {
        "authenticate"
    }

    fn description() -> &'static str {
        "Authenticates to a registry and and adds a token text attribute."
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|_| {
            let mut tc = context.clone();
            async move {
                if let Some(credentials) = Self::authenticate(&tc).await {
                    event!(Level::DEBUG, "Received credentials for registry");
                    tc.as_mut()
                        .with_text(
                            "refresh_token",
                            credentials
                                .refresh_token
                                .expect("received some refresh token"),
                        )
                        .add_text_attr(
                            "access_token",
                            credentials
                                .access_token
                                .expect("received some access token"),
                        );

                    Some(tc)
                } else {
                    event!(Level::ERROR, "Could not authn w/ registry");
                    None
                }
            }
        })
    }
}

#[test]
fn test_resolve_challenge() {
    let url = Authenticate::parse_challenge_header(
        r#"Bearer realm="https://host.io/oauth2/token",service="host.io",scope="repository:hello-world:pull""#,
    );
    assert_eq!(
        url,
        "https://host.io/oauth2/token?service=host.io&scope=repository:hello-world:pull"
    )
}
