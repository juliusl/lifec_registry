use lifec::prelude::{AttributeParser, BlockProperties, CustomAttribute};
use lifec::prelude::{BlockObject, Value, ThunkContext, AsyncContext, AttributeIndex, Plugin, Resources, Process};
use rust_embed::RustEmbed;
use tokio::select;
use tracing::event;
use tracing::Level;

/// Plugin to handle signing into azure,
///
#[derive(RustEmbed, Default)]
#[folder = "lib/sh/"]
#[include = "login-acr.sh"]
#[include = "login-acr-admin.sh"]
pub struct LoginACR;

impl LoginACR {
    fn login_access_token(registry_name: impl AsRef<str>, tc: &mut ThunkContext) -> AsyncContext {
        tc.state_mut()
            .with_symbol("process", "sh login-acr.sh")
            .with_symbol("env", "REGISTRY_NAME")
            .with_symbol("REGISTRY_NAME", registry_name.as_ref());
            
        Process::call(&tc).expect("Should start")
    }

    fn login_admin(registry_name: impl AsRef<str>, tc: &mut ThunkContext) -> AsyncContext {
        tc.state_mut()
            .with_symbol("process", "sh login-acr-admin.sh")
            .with_symbol("env", "REGISTRY_NAME")
            .with_symbol("REGISTRY_NAME", registry_name.as_ref());
            
        Process::call(&tc).expect("Should start")
    }
}

impl Plugin for LoginACR {
    fn symbol() -> &'static str {
        "login-acr"
    }

    fn description() -> &'static str {
        "Calls a login script, and outputs an access_token"
    }

    fn call(context: &ThunkContext) -> Option<AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                Resources("")
                    .unpack_resource::<LoginACR>(&tc, &String::from("login-acr.sh"))
                    .await;
                
                Resources("")
                    .unpack_resource::<LoginACR>(&tc, &String::from("login-acr-admin.sh"))
                    .await;

                if let Some(registry_name) = tc.state().find_symbol("login-acr") {
                    let admin_enabled = tc.state().find_bool("admin").unwrap_or_default();
                    let login_process = if admin_enabled {
                        Self::login_admin(&registry_name, &mut tc)
                    } else {
                        Self::login_access_token(&registry_name, &mut tc)
                    };
                
                    let (task, cancel) = login_process;
                    select! {
                        tc = task => {
                            event!(Level::DEBUG, "Finished login to acr - {}", registry_name);
                            if let Some(mut tc) = tc.ok() {
                                let registry_host = tc.state().find_symbol("host").unwrap_or("azurecr.io".to_string());
                                if admin_enabled {
                                    let registry = format!("{registry_name}.{registry_host}");
                                    let username = format!("{registry}.username");
                                    tc.state_mut()
                                        .with_symbol(username, &registry_name)
                                        .with_symbol(
                                            format!("{registry_name}.{registry_host}"), 
                                            tokio::fs::read_to_string("admin_pass").await.expect("a file should have been created").trim().trim_matches('"')
                                    );
                                } else {
                                    let registry = format!("{registry_name}.{registry_host}");
                                    let username = format!("{registry}.username");
                                    tc.state_mut()
                                        .with_symbol(username, "00000000-0000-0000-0000-000000000000")
                                        .with_symbol(
                                            registry, 
                                            tokio::fs::read_to_string("access_token").await.expect("a file should have been created").trim()
                                    );
                                }

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

    fn compile(parser: &mut AttributeParser) {
        parser.add_custom_with("host", |p, content| {
            if let Some(last_entity) = p.last_child_entity() {
                p.define_child(last_entity, "host", Value::Symbol(content));
            }
        });

        parser.add_custom_with("admin", |p, _| {
            if let Some(last_entity) = p.last_child_entity() {
                p.define_child(last_entity, "admin", true);
            }
        });
    }
}

impl BlockObject for LoginACR {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
            .require("login-acr")
            .optional("windows")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}