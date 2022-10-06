use lifec::{AttributeIndex, BlockObject, BlockProperties, Plugin, Value};
use serde_json::json;
use tracing::event;
use tracing::Level;

/// Plugin that handles setting up the registry credentials for overlaybd
///
#[derive(Default)]
pub struct LoginOverlayBD;

impl Plugin for LoginOverlayBD {
    fn symbol() -> &'static str {
        "login-overlaybd"
    }

    fn call(context: &lifec::ThunkContext) -> Option<lifec::AsyncContext> {
        context.task(|_| {
            let tc = context.clone();
            async {
                if let Some(creds_path) = tc.search().find_symbol("login-overlaybd") {
                    match tokio::fs::read_to_string(&creds_path).await {
                        Ok(content) => {
                            let mut value =
                                serde_json::from_str::<serde_json::Value>(content.as_str())
                                    .expect("should be valid json");

                            if let Some(auths) = value
                                .as_object_mut()
                                .and_then(|f| f.get_mut("auths"))
                                .and_then(|a| a.as_object_mut())
                            {
                                for registry in tc.search().find_symbol_values("registry") {
                                    if !auths.contains_key(&registry) {
                                        if let Some(cred) = tc.search().find_symbol(&registry) {
                                            let user_name = tc.search().find_symbol(format!("{registry}.username")).expect("should have a username");
                                            let creds = json!({
                                                "username": user_name,
                                                "password": cred
                                            });

                                            auths.insert(registry, creds);
                                        }
                                    }
                                }
                            }

                            match serde_json::to_string_pretty(&value) {
                                Ok(auths) => match tokio::fs::write(creds_path, auths).await {
                                    Ok(_) => {
                                        event!(Level::DEBUG, "Wrote to overlaybd's cred file");
                                    }
                                    Err(err) => {
                                        event!(Level::ERROR, "Could not write to file, {err}")
                                    }
                                },
                                Err(err) => {
                                    event!(Level::ERROR, "Could not serialize auth map, {err}")
                                }
                            }
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Could not read overlaybd cred file, {err}")
                        },
                    }
                }

                Some(tc)
            }
        })
    }

    fn compile(parser: &mut lifec::AttributeParser) {
        parser.add_custom_with("registry", |p, content| {
            if let Some(last_entity) = p.last_child_entity() {
                p.define_child(last_entity, "registry", Value::Symbol(content));
            }
        });
    }
}

impl BlockObject for LoginOverlayBD {
    fn query(&self) -> lifec::BlockProperties {
        BlockProperties::default().require("login-overlaybd")
    }

    fn parser(&self) -> Option<lifec::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
