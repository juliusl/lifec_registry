use lifec::prelude::{AttributeParser, BlockProperties, CustomAttribute};
use lifec::prelude::{BlockObject, ThunkContext, AsyncContext, AttributeIndex, Plugin, Resources, Process};
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
    /// Fetch an ACR refresh token (Named access_token for historical reasons),
    /// 
    fn login_access_token(registry_name: impl AsRef<str>, tc: &mut ThunkContext) -> AsyncContext {
        tc.state_mut()
            .with_symbol("process", "sh login-acr.sh")
            .with_symbol("env", "REGISTRY_TENANT")
            .with_symbol("REGISTRY_TENANT", registry_name.as_ref());
            
        Process::call(tc).expect("Should start")
    }

    /// Fetch admin credentials,
    /// 
    fn login_admin(registry_name: impl AsRef<str>, tc: &mut ThunkContext) -> AsyncContext {
        tc.state_mut()
            .with_symbol("process", "sh login-acr-admin.sh")
            .with_symbol("env", "REGISTRY_TENANT")
            .with_symbol("REGISTRY_TENANT", registry_name.as_ref());
            
        Process::call(tc).expect("Should start")
    }
}

impl Plugin for LoginACR {
    fn symbol() -> &'static str {
        "login_acr"
    }

    fn description() -> &'static str {
        "Calls a login script, and outputs an access_token"
    }

    fn call(context: &mut ThunkContext) -> Option<AsyncContext> {
        context.task(|cancel_source| {
            let mut tc = context.clone();
            async move {
                Resources("")
                    .unpack_resource::<LoginACR>(&tc, &String::from("login-acr.sh"))
                    .await;
                
                Resources("")
                    .unpack_resource::<LoginACR>(&tc, &String::from("login-acr-admin.sh"))
                    .await;

                let registry = tc.workspace().expect("should have a workspace").get_tenant().expect("should have a tenant").clone();
                let admin_enabled = tc.state().find_bool("admin").unwrap_or_default();
                
                let (task, cancel) = if admin_enabled {
                    Self::login_admin(&registry, &mut tc)
                } else {
                    Self::login_access_token(&registry, &mut tc)
                };
                
                select! {
                    tc = task => {
                        event!(Level::DEBUG, "Finished login to acr - {}", registry);
                        if let Some(tc) = tc.ok() {
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
            }
        })
    }

    fn compile(parser: &mut AttributeParser) {
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
            .optional("admin")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}