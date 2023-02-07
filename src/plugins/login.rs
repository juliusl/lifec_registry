use std::path::PathBuf;

use lifec::prelude::{
    AttributeIndex, BlockObject, BlockProperties, Component, CustomAttribute, DenseVecStorage,
    Plugin, ThunkContext,
};
use tracing::{debug, warn};

use crate::{default_access_provider, Error, OAuthToken};

/// Component to login to a registry,
///
/// Reads token from file_src in the work directory,
///
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Login;

impl Login {
    /// Resets the token cache,
    /// 
    async fn reset_cache(token_src: &PathBuf) -> Result<(), Error> {
        tokio::fs::remove_file(&token_src).await?;

        Ok(())
    }

    /// Parses token from the current state,
    /// 
    async fn parse_token(token_src: &PathBuf, tc: &ThunkContext) -> Result<String, Error> {
        match token_src.canonicalize() {
            Ok(path) => {
                let cached = Self::read_token_cache(&path).await?;
                Ok(cached.token())
            },
            Err(ref err)
                if err.raw_os_error() == Some(2)
                    && tc.client().is_some()
                    && tc.search().find_symbol("api").is_some() =>
            {
                if let (Some(client), Some(api)) =
                    (tc.client(), tc.search().find_symbol("api"))
                {
                    let access_provider = default_access_provider(None::<PathBuf>);
                    let access_token = access_provider
                        .access_token()
                        .await?;

                    let refresh_token = OAuthToken::refresh_token(
                        client,
                        api,
                        access_token,
                        access_provider.tenant_id(),
                    )
                    .await?;

                    let token = refresh_token.token();
                    Self::cache_token(token_src, &refresh_token).await?;
                    
                    Ok(token)
                } else {
                    return Err(Error::invalid_operation(
                        "cannot generate refresh_token, missing deps",
                    ));
                }
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    /// Write to the token cache,
    /// 
    async fn cache_token(token_src: &PathBuf, token: &OAuthToken) -> Result<(), Error> {
        tokio::fs::write(token_src, serde_json::to_string_pretty(token)?).await?;

        Ok(())
    }

    /// Read from the token cache,
    /// 
    async fn read_token_cache(token_src: &PathBuf) -> Result<OAuthToken, Error> {
        let cached = tokio::fs::read_to_string(token_src).await?;

        let token = serde_json::from_str::<OAuthToken>(cached.as_str())?;

        if token.is_expired()? {
            Err(Error::recoverable_error("token has expired, reissue required"))
        } else {
            Ok(token)
        }
    }
}

impl Plugin for Login {
    fn symbol() -> &'static str {
        "login"
    }

    fn description() -> &'static str {
        "Login to a registry, adds a `user` and `token` text attribute"
    }

    fn call(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.task_with_result(|_| {
            let mut tc = context.clone();
            async {
                debug!("Starting registry login");
                if let Some(token_src) = tc.state().find_symbol("login") {
                    let token_src = &token_src;

                    let token_src = tc
                        .work_dir()
                        .expect("should have a work dir")
                        .join(token_src);

                    let token = match Self::parse_token(&token_src, &tc).await {
                        Ok(token) => {
                            token
                        },
                        Err(ref err) if err.is_recoverable() => {
                            Self::reset_cache(&token_src).await?;
                            
                            Self::parse_token(&token_src, &tc).await?
                        },
                        Err(err) => {
                            return Err(err.into());
                        }
                    };

                    debug!("Writing credentials to context");
                    tc.state_mut()
                        .with_symbol("REGISTRY_USER", "00000000-0000-0000-0000-000000000000")
                        .with_symbol("REGISTRY_TOKEN", token.trim());
                } else {
                    warn!("Missing login property");
                }

                tc.copy_previous();
                Ok(tc)
            }
        })
    }
}

impl BlockObject for Login {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
            .require("login")
            .optional("REGISTRY_USER")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Login::as_custom_attr())
    }
}
