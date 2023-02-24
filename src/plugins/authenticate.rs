use hyper::{http, Method, Uri};
use lifec::prelude::{
    AttributeIndex, BlockObject, BlockProperties, Component, CustomAttribute, DenseVecStorage,
    Plugin, ThunkContext, Value,
};
use poem::{web::headers::Authorization, Request};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{debug, error, info, trace, warn};

/// Plugin for authenticating w/ a registry
///
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Authenticate;

/// Struct for token response when authenticating
///
#[derive(Deserialize, Serialize)]
pub struct Credentials {
    access_token: Option<String>,
    refresh_token: Option<String>,
}

impl Plugin for Authenticate {
    fn symbol() -> &'static str {
        "authn"
    }

    fn description() -> &'static str {
        "Authenticates to a registry and and adds an `access_token` to state"
    }

    fn call(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|_| {
            let mut tc = context.clone();
            async move {
                if tc.search().find_symbol("api").is_none() {
                    if let Some(authn) = tc.search().find_symbol("authn") {
                        if !authn.is_empty() {
                            tc.state_mut().with_symbol("api", authn);
                        }
                    }
                }

                if let Some(credentials) = Self::authenticate(&tc).await {
                    match Authorization::bearer(
                        credentials
                            .access_token
                            .expect("received some access token")
                            .as_str(),
                    ) {
                        Ok(auth_header) => {
                            tc.state_mut()
                                .with_symbol("header", "Authorization")
                                .with_symbol(
                                    "Authorization",
                                    format!("Bearer {}", auth_header.token()),
                                );
                        }
                        Err(err) => {
                            error!("Could not parse auth header, {err}");
                        }
                    }

                    tc.copy_previous();
                    Some(tc)
                } else {
                    error!("Could not authn w/ registry");
                    None
                }
            }
        })
    }

    fn compile(parser: &mut lifec::prelude::AttributeParser) {
        parser.add_custom_with("method", |p, content| {
            let entity = p.last_child_entity().expect("should have an entity");

            p.define_child(entity, "method", Value::Symbol(content.to_uppercase()));
        });
    }
}

impl BlockObject for Authenticate {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
            .require("authn")
            .require("ns")
            .require("api")
            .require("token")
            .require("method")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

impl Authenticate {
    /// Authenticates the request to the registry and returns credentials
    ///
    /// Required Properties:
    /// ns, symbol
    /// token, symbol
    ///
    async fn authenticate(tc: &ThunkContext) -> Option<Credentials> {
        if let Some(challenge_uri) = Self::start_challenge(tc).await {
            let (ns, req) = if let (Some(ns), Some(user), Some(password)) = (
                tc.search().find_symbol("REGISTRY_NAMESPACE"),
                tc.search().find_symbol("REGISTRY_USER"),
                tc.search().find_symbol("REGISTRY_PASSWORD"),
            ) {
                info!("Start authn for {challenge_uri} w/ login config");
                /*
                # Example curl request:
                curl -v -X POST -H "Content-Type: application/x-www-form-urlencoded" -d \
                "grant_type=password&service=$registry&scope=$scope&username=$acr_user&password=&acr_passwd" \
                https://$registry/oauth2/token
                */

                if let Ok(encoded) = serde_urlencoded::to_string(&[
                    ("grant_type", "password"),
                    ("username", user.as_str()),
                    ("password", password.as_str()),
                ]) {
                    let body = format!("{}&{}", challenge_uri.query().unwrap(), encoded);
                    let req = Request::builder()
                        .uri(challenge_uri)
                        .header("Content-Type", "application/x-www-form-urlencoded")
                        .method(Method::POST)
                        .body(body);
                    (ns, req)
                } else {
                    tracing::error!("Could not encode username/password authn body");
                    return None;
                }
            } else if let (Some(ns), Some(token)) = (
                tc.search().find_symbol("REGISTRY_NAMESPACE"),
                tc.search().find_symbol("REGISTRY_TOKEN"),
            ) {
                info!("Start authn for {challenge_uri}");

                /*
                # Example curl request:
                curl -v -X POST -H "Content-Type: application/x-www-form-urlencoded" -d \
                "grant_type=refresh_token&service=$registry&scope=$scope&refresh_token=$acr_refresh_token" \
                https://$registry/oauth2/token
                */

                let body = format!(
                    "{}&grant_type=refresh_token&refresh_token={}",
                    challenge_uri.query().unwrap(),
                    token
                );

                let req = Request::builder()
                    .uri(challenge_uri)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .method(Method::POST)
                    .body(body);

                (ns, req)
            } else {
                (String::new(), Request::default())
            };

            if ns.is_empty() {
                tracing::error!("Tried to authn w/o credentials");
                return None;
            }

            let client = tc
                .client()
                .expect("async is enabled, so this should be set");

            trace!("{:#?}", req);
            match client.request(req.into()).await {
                Ok(response) => {
                    trace!("{:#?}", response);
                    match hyper::body::to_bytes(response.into_body()).await {
                        Ok(bytes) => {
                            return serde_json::de::from_slice::<Credentials>(bytes.as_ref()).ok()
                        }
                        Err(err) => {
                            error!("Could not decode credentials, {ns} {err}")
                        }
                    }
                }
                Err(err) => error!("Could not fetch credentials for, {ns} {err}"),
            }
        }

        None
    }

    /// Gets the challenge header from the registry
    ///
    /// Required Properties:
    /// api: symbol
    ///
    async fn start_challenge(tc: &ThunkContext) -> Option<Uri> {
        if let Some(client) = tc.client() {
            let api = tc
                .search()
                .find_symbol("api")
                .and_then(|a| Uri::from_str(a.as_str()).ok());

            if let Some(api) = api {
                info!("calling {api} to initiate authn");
                let method = tc
                    .search()
                    .find_symbol("method")
                    .expect("should have a method");

                let request = Request::builder()
                    .uri(api)
                    .method(
                        Method::from_bytes(method.to_string().to_uppercase().as_bytes())
                            .expect("should be able to parse"),
                    )
                    .finish();

                if let Some(response) = client.request(request.into()).await.ok() {
                    if response.status().is_client_error() {
                        debug!("client error detected, starting auth challenge");
                        trace!("{:#?}", response);
                        let challenge = response
                            .headers()
                            .get(http::header::WWW_AUTHENTICATE)
                            .expect("401 should've been returned w/ a challenge header");
                        let challenge = challenge
                            .to_str()
                            .expect("challenge header should be a string");
                        let challenge = Self::parse_challenge_header(challenge);

                        debug!("received challange {challenge}");
                        return Some(
                            Uri::from_str(&challenge).expect("challenge should be a valid uri"),
                        );
                    }
                }
            }
        }

        warn!("Did not authn request, exiting, {:?}", tc.client());
        None
    }

    fn parse_challenge_header(challenge: impl AsRef<str>) -> String {
        challenge
            .as_ref()
            .trim_start_matches(r#"Bearer realm=""#)
            .replace(r#"",service="#, r#"?service="#)
            .replace(",", "&")
            .replace('"', "")
            // TODO fix this later
            .replace("pull&push", "pull,push")
            .replace("push&pull", "push,pull")
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
