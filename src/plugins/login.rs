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

    fn call(context: &ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async {      
                event!(Level::DEBUG, "Starting registry login");
                if let Some(token_src) = tc.state().find_symbol("file_src") {
                    let token_src = &token_src;
                    event!(Level::DEBUG, "Found file_src for token at {token_src}");
                    let user = tc
                        .state()
                        .find_symbol("user")
                        .unwrap_or("00000000-0000-0000-0000-000000000000".to_string());
                    match tokio::fs::read_to_string(token_src).await {
                        Ok(token) => {
                            event!(Level::DEBUG, "Writing credentials to context");
                            tc.state_mut()
                                .with_text("user", user)
                                .with_text("token", token.trim());
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Issue reading {token_src} -- {err}");
                        }
                    }
                } else {
                    event!(Level::WARN, "Missing file_src property");
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
            .require("file_src")
            .require("login")
            .optional("user")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Login::as_custom_attr())
    }
}
