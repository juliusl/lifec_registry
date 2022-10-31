use lifec::prelude::{
    Plugin, ThunkContext,
    Component, DenseVecStorage,
    AttributeIndex, BlockObject, BlockProperties, CustomAttribute,
};
use tracing::{event, Level};

mod acr;
pub use acr::LoginACR;

mod overlaybd;
pub use overlaybd::LoginOverlayBD;

/// Component to login to a registry, 
/// 
/// Reads token from file_src in the work directory,
/// 
#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Login;

impl Plugin for Login {
    fn symbol() -> &'static str {
        "login"
    }

    fn description() -> &'static str {
        "Login to a registry, adds a `user` and `token` text attribute"
    }

    fn call(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async {      
                event!(Level::DEBUG, "Starting registry login");
                if let Some(token_src) = tc.state().find_symbol("login") {
                    let token_src = &token_src;
                    event!(Level::DEBUG, "login method: {token_src}");
                    let user = tc
                        .state()
                        .find_symbol("REGISTRY_USER")
                        .unwrap_or("00000000-0000-0000-0000-000000000000".to_string());
                    match tokio::fs::read_to_string(token_src).await {
                        Ok(token) => {
                            event!(Level::DEBUG, "Writing credentials to context");
                            tc.state_mut()
                                .with_symbol("REGISTRY_USER", user)
                                .with_symbol("REGISTRY_TOKEN", token.trim());
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Issue reading {token_src} -- {err}");
                        }
                    }
                } else {
                    event!(Level::WARN, "Missing login property");
                }

                tc.copy_previous();

                Some(tc)
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
