use std::io::ErrorKind;

use hyper::{http::HeaderValue, Method, Body};
use serde::{Deserialize, Serialize};

/// Struct representing docker's oauth config specification,
///
/// The following documentation for fields are from: https://docs.docker.com/registry/spec/auth/oauth/
///
#[derive(Serialize, Deserialize)]
pub struct OAuthConfig {
    /// OAuth2 realm that provides the token,
    ///
    #[serde(skip)]
    realm: String,
    /// Type of grant used to get token.
    ///
    /// For oauth2/token, When getting a refresh token using credentials this type should be set to "password" and have the accompanying username and password parameters.
    /// Type "authorization_code" is reserved for future use for authenticating to an authorization server without having to send credentials directly from the client.
    /// When requesting an access token with a refresh token this should be set to "refresh_token".
    ///
    /// For oauth2/exchange 'access_token' can be a possible value.
    ///
    grant_type: String,
    /// The name of the service which hosts the resource to get access for. Refresh tokens will only be good for getting tokens for this service.
    ///
    service: String,
    /// String identifying the client. This client_id does not need to be registered with the authorization server but should be set to a meaningful value in order to allow auditing keys created by unregistered clients.
    /// Accepted syntax is defined in RFC6749 Appendix A.1.
    ///
    /// Note: Even though the spec says that this field is required, in practice at least for acr it is not required
    ///
    #[serde(skip_serializing_if = "Option::is_none")]
    client_id: Option<String>,
    /// Tenant id for the /exchange api, only used if grant_type is `access_token`
    /// 
    #[serde(skip_serializing_if = "Option::is_none")]
    tenant: Option<String>,
    /// Access which is being requested. If "offline" is provided then a refresh token will be returned. The default is "online" only returning short lived access token.
    /// If the grant type is "refresh_token" this will only return the same refresh token and not a new one.
    ///
    #[serde(skip_serializing_if = "Option::is_none")]
    access_type: Option<String>,
    /// The resource in question, formatted as one of the space-delimited entries from the scope parameters from the WWW-Authenticate header shown above.
    /// This query parameter should only be specified once but may contain multiple scopes using the scope list format defined in the scope grammar.
    /// If multiple scope is provided from WWW-Authenticate header the scopes should first be converted to a scope list before requesting the token. The above example would be specified as: scope=repository:samalba/my-app:push.
    /// When requesting a refresh token the scopes may be empty since the refresh token will not be limited by this scope, only the provided short lived access token will have the scope limitation.
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    /// The refresh token to use for authentication when grant type "refresh_token" is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    /// The access token to use for authentication when grant type "access_token" is used. Only used w/ oauth2/exchange.
    #[serde(skip_serializing_if = "Option::is_none")]
    access_token: Option<String>,
    /// The username to use for authentication when grant type "password" is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    /// The password to use for authentication when grant type "password" is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

impl OAuthConfig {
    /// Consumes the OAuth2 config and builds a request w/ this config,
    /// 
    /// If `grant_type` of access_token is used, then realm will switch to /oauth2/exchange
    /// 
    pub fn build_request(mut self) -> Result<hyper::Request<Body>, std::io::Error> {
        let mut uri = self.realm.clone();

        if self.tenant.is_some() {
            uri = uri.replace("token", "exchange");
            self.scope.take();
        }

        let body = serde_urlencoded::to_string(self).map_err(|e| std::io::Error::new(ErrorKind::InvalidInput, e))?;

        hyper::Request::builder()
            .uri(uri)
            .method(Method::POST)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
    }

    /// Sets the client id field, chainable
    /// 
    #[inline]
    pub fn client_id(mut self, client_id: String) -> Self {
        self.client_id = Some(client_id);
        self
    }

    /// Sets the access_type field, chainable
    /// 
    #[inline]
    pub fn access_type(mut self, access_type: String) -> Self {
        self.access_type = Some(access_type);
        self
    }
}

/// Struct that reprsents the Www-Authenticate header in Bearer mode,
/// 
#[derive(Serialize, Deserialize, Clone)]
pub struct BearerChallengeConfig {
    /// OAuth2 realm to request a token from,
    /// 
    realm: String,
    /// Host that is issuing the challenge
    /// 
    service: String,
    /// Scope of the token required to complete the challenge,
    /// 
    #[serde( skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

impl BearerChallengeConfig {
    /// Parses a Www-Authenticate header into a BearerChallengeConfig,
    ///
    pub fn parse_from_header(header_value: &HeaderValue) -> Result<Self, std::io::Error> {
        let value = header_value
            .to_str()
            .map_err(|e| std::io::Error::new(ErrorKind::InvalidInput, e))?;

        if !value.starts_with("Bearer") {
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "header is not bearer auth",
            ));
        }

        serde_urlencoded::from_str::<BearerChallengeConfig>(
            value
                .trim_start_matches("Bearer")
                .trim()
                .replace("\"", "")
                .replace(",", "&")
                .replace("pull&", "pull,")
                .replace("push&", "push,")
                .replace("delete&", "delete,")
                .replace("metadata_read&", "metadata_read,")
                .replace("metadata_write&", "metadata_write,")
                .as_str(),
        )
        .map_err(|e| std::io::Error::new(ErrorKind::InvalidInput, e))
    }

    /// Consumes the challenge and returns an OAuthConfig for exchanging an access_token for a refresh_token
    ///
    pub fn exchange(self, access_token: impl Into<String>, tenant_id: impl Into<String>) -> OAuthConfig {
        OAuthConfig {
            realm: self.realm,
            grant_type: String::from("access_token"),
            service: self.service,
            access_token: Some(access_token.into()),
            tenant: Some(tenant_id.into()),
            scope: None,
            refresh_token: None,
            client_id: None,
            access_type: None,
            username: None,
            password: None,
        }
    }

    /// Consumes the challenge and returns an OAuthConfig for exchanging a username/password for a refresh_token
    /// 
    pub fn exchange_by_password(self, username: impl Into<String>, password: impl Into<String>, tenant_id: impl Into<String>) -> OAuthConfig {
        OAuthConfig {
            realm: self.realm,
            grant_type: String::from("password"),
            service: self.service,
            tenant: Some(tenant_id.into()),
            username: Some(username.into()),
            password: Some(password.into()),
            access_token: None,
            scope: None,
            refresh_token: None,
            client_id: None,
            access_type: None,
        }
    }

    /// Consumes the challenge config and returns an OAuthConfig for receiving a token by refresh_token grant
    ///
    pub fn token_by_refresh_token(self, refresh_token: impl Into<String>) -> OAuthConfig {
        OAuthConfig {
            realm: self.realm,
            grant_type: String::from("refresh_token"),
            service: self.service,
            scope: self.scope,
            refresh_token: Some(refresh_token.into()),
            tenant: None,
            access_token: None,
            client_id: None,
            access_type: None,
            username: None,
            password: None,
        }
    }

    /// Consumes the challenge config and returns an OAuthConfig for receiving a token by password grant
    ///
    pub fn token_by_password(
        self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> OAuthConfig {
        OAuthConfig {
            realm: self.realm,
            grant_type: String::from("password"),
            service: self.service,
            scope: self.scope,
            username: Some(username.into()),
            password: Some(password.into()),
            tenant: None,
            client_id: None,
            access_type: None,
            refresh_token: None,
            access_token: None,
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use hyper::Body;

    #[tokio::test]
    async fn test_bearer_challenge_config() {
        use super::BearerChallengeConfig;
        use hyper::http::HeaderValue;

        let challenge = r#"Bearer realm="https://host.io/oauth2/token",service="host.io",scope="repository:hello-world:pull,push""#;

        let config = BearerChallengeConfig::parse_from_header(&HeaderValue::from_static(challenge))
            .expect("should be able to parse config");

        assert_eq!("https://host.io/oauth2/token", config.realm);
        assert_eq!("host.io", config.service);
        assert_eq!(Some("repository:hello-world:pull,push".to_string()), config.scope);

        async fn convert_to_string(body: &mut Body) -> String {
            let bytes = hyper::body::to_bytes(body).await.unwrap();

            String::from_utf8(bytes.to_vec()).unwrap()
        }

        // Test refresh_token request generation
        let oauth_config = config.clone().token_by_refresh_token("testtoken");
        let mut request = oauth_config.build_request().expect("should be able to generate request");
        assert_eq!("grant_type=refresh_token&service=host.io&scope=repository%3Ahello-world%3Apull%2Cpush&refresh_token=testtoken", convert_to_string(request.body_mut()).await);

        // Test password request generation
        let oauth_config = config.clone().token_by_password("testusername", "testpassword");
        let mut request = oauth_config.build_request().expect("should be able to generate request");
        assert_eq!("grant_type=password&service=host.io&scope=repository%3Ahello-world%3Apull%2Cpush&username=testusername&password=testpassword", convert_to_string(request.body_mut()).await);

        // Test exchange request generation
        let oauth_config = config.clone().exchange("testaccesstoken", "testtenant");
        let mut request = oauth_config.build_request().expect("should be able to generate request");
        assert_eq!("grant_type=access_token&service=host.io&tenant=testtenant&access_token=testaccesstoken", convert_to_string(request.body_mut()).await);
    }
}
