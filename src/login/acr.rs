use lifec::{BlockObject, Value};
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
        "Calls a login script, and outputs an access_token"
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
                        
                    let registry_host = tc.state().find_symbol("host").unwrap_or("azurecr.io".to_string());
                
                    let (task, cancel) = Process::call(&tc).expect("Should start");
                    select! {
                        tc = task => {
                            event!(Level::DEBUG, "Finished login to acr - {}", registry_name);
                            if let Some(mut tc) = tc.ok() {
                                tc.state_mut()
                                    .with_symbol(
                                        format!("{registry_name}.{registry_host}"), 
                                        tokio::fs::read_to_string("access_token").await.expect("a file should have been created")
                                    );

                                Some(tc)
                            } else {
                                None
                            }
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

    fn compile(parser: &mut lifec::AttributeParser) {
        parser.add_custom_with("host", |p, content| {
            if let Some(last_entity) = p.last_child_entity() {
                p.define_child(last_entity, "host", Value::Symbol(content));
            }
        });
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