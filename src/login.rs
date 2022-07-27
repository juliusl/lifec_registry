use lifec::{plugins::{Plugin, ThunkContext}, DenseVecStorage, Component};
use tracing::{event, Level};

#[derive(Component, Default)]
#[storage(DenseVecStorage)]
pub struct Login;

impl Plugin<ThunkContext> for Login {
    fn symbol() -> &'static str {
        "login"
    }

    fn description() -> &'static str {
        "Login to a registry, adds a `user` and `token` text attribute"
    }

    fn call_with_context(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.clone().task(|_|{
            let mut tc = context.clone();
            async {
                // TODO: Check multiple sources
                if let Some(token_src) = tc.as_ref().find_text("token_src") {
                    match tokio::fs::read_to_string(token_src).await {
                        Ok(token) => {
                            tc.as_mut()
                                .with_text("user", "00000000-0000-0000-0000-000000000000")
                                .add_text_attr("token", token);
                        },
                        Err(err) => {
                            event!(Level::ERROR, "{err}");
                        },
                    }
                }
                
                Some(tc)
            }
        })

    }
}