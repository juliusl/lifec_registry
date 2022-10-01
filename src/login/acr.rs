use lifec::BlockObject;
use lifec::{AttributeIndex, Plugin, Resources, Process};
use rust_embed::RustEmbed;
use tokio::select;
use tracing::event;
use tracing::Level;

/// Plugin to handle signing into azure,
///
#[derive(RustEmbed, Default)]
#[folder = "lib/sh/"]
#[include = "login-acr.sh"]
pub struct LoginACR;

impl Plugin for LoginACR {
    fn symbol() -> &'static str {
        "login-acr"
    }

    fn description() -> &'static str {
        "Calls a login script, and outputs an access_token to world_dir"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                Resources("")
                    .unpack_resource::<LoginACR>(&tc, &String::from("login-acr.sh"))
                    .await;

                if let Some(registry_name) = tc.state().find_symbol("login-acr") {
                    event!(Level::DEBUG, "Finished login to acr - {}", registry_name);

                    tc.state_mut()
                        .with_symbol("process", "sh login-acr.sh")
                        .with_symbol("env", "REGISTRY_NAME")
                        .with_symbol("REGISTRY_NAME", &registry_name);
                
                    let (task, cancel) = Process::call(&tc).expect("Should start");
                    select! {
                        tc = task => {
                            event!(Level::DEBUG, "Finished login to acr - {}", registry_name);
                            tc.ok()
                        }
                        _ = cancel_source => {
                            cancel.send(()).ok();
                            None
                        }
                    }
                } else {
                    panic!("A registry name was not provided, cannot continue to logging in");
                }
            }
        })
    }
}

impl BlockObject for LoginACR {
    fn query(&self) -> lifec::BlockProperties {
        lifec::BlockProperties::default()
            .require("login-acr")
            .optional("windows")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}