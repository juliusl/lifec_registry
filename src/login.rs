use lifec::{
    plugins::{Plugin, ThunkContext},
    Component, DenseVecStorage,
};
use tracing::{event, Level};

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

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|_| {
            let mut tc = context.clone();
            async {
                // TODO: Check multiple sources
                if let Some(token_src) = tc.as_ref().find_text("file_src") {
                    let user = tc
                        .as_ref()
                        .find_text("user")
                        .unwrap_or("00000000-0000-0000-0000-000000000000".to_string());
                    match tokio::fs::read_to_string(token_src).await {
                        Ok(token) => {
                            tc.as_mut()
                                .with_text("user", user)
                                .add_text_attr("token", token);
                        }
                        Err(err) => {
                            event!(Level::ERROR, "{err}");
                        }
                    }
                }

                Some(tc)
            }
        })
    }
}
