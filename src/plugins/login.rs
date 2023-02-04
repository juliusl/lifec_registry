use std::path::PathBuf;

use lifec::prelude::{
    AttributeIndex, BlockObject, BlockProperties, Component, CustomAttribute, DenseVecStorage,
    Plugin, ThunkContext,
};
use tracing::{debug, warn};

mod acr;
pub use acr::LoginACR;

mod overlaybd;
pub use overlaybd::LoginOverlayBD;

mod nydus;
pub use nydus::LoginNydus;

use crate::{default_access_provider, Error, OAuthToken};

/// Component to login to a registry,
///
/// Reads token from file_src in the work directory,
///
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Login;

impl Login {
    /// Parses token from the current state,
    /// 
    async fn parse_token(token_src: PathBuf, tc: &ThunkContext) -> Result<String, Error> {
        match token_src.canonicalize() {
            Ok(path) => Ok(tokio::fs::read_to_string(path).await?),
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
                    tokio::fs::write(token_src, &token).await?;
                    
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

                    let token = Self::parse_token(token_src, &tc).await?;

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
