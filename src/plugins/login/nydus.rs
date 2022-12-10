use lifec::{prelude::{Plugin, ThunkContext, AsyncContext, AttributeParser, Value, BlockObject, BlockProperties, CustomAttribute, AddDoc}, state::AttributeIndex};
use serde_json::json;
use tracing::{event, Level};

/// Plugin that handles setting up the registry credentials for nydus
/// 
/// Nydusd/Nydusify needs a docker config file to handle sign-in
///
#[derive(Default)]
pub struct LoginNydus;


impl Plugin for LoginNydus {
    fn symbol() -> &'static str {
        "login_nydus"
    }

    fn call(context: &mut ThunkContext) -> Option<AsyncContext> {
        context.task(|_| {
            let tc = context.clone();
            async {
                if let Some(creds_path) = tc.search().find_symbol("login_nydus") {
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

                            match serde_json::to_string_pretty(&value) {
                                Ok(auths) => match tokio::fs::write(creds_path, auths).await {
                                    Ok(_) => {
                                        event!(Level::DEBUG, "Wrote to docker's cred file");
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
                            event!(Level::ERROR, "Could not read docker cred file, {err}")
                        },
                    }
                }

                Some(tc)
            }
        })
    }

    fn compile(parser: &mut AttributeParser) {
        if let Some(mut docs) = Self::start_docs(parser) {
            let docs = &mut docs;

            docs.as_mut().add_custom_with("registry", |p, content| {
                if let Some(last_entity) = p.last_child_entity() {
                    p.define_child(last_entity, "registry", Value::Symbol(content));
                }
            })
            .add_doc(docs, "The registry to login with nydus");
        }
    }
}

impl BlockObject for LoginNydus {
    fn query(&self) -> BlockProperties {
        BlockProperties::default().require("login_nydus")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}


