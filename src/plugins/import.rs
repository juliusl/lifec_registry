

use lifec::prelude::{Plugin, BlockObject, BlockProperties, AttributeIndex, Process, Resources, Value, AsyncContext, ThunkContext, CustomAttribute, AttributeParser};
use logos::Logos;
use poem::Request;
use rust_embed::RustEmbed;
use tokio::select;
use tracing::{event, Level};

use crate::{proxy::ProxyTarget, Platform, ImageIndex};


/// Plugin to handle importing a public source image to a private repo
/// 
#[derive(Default, RustEmbed)]
#[folder = "lib/sh/"]
#[include = "import.sh"]
pub struct Import;

impl Plugin for Import {
    fn symbol() -> &'static str {
        "import"
    }

    fn call(context: &ThunkContext) -> Option<AsyncContext> {
        context.task(|cancel_source|{
            let mut tc = context.clone();
            async {
                Resources("")
                    .unpack_resource::<Import>(&tc, &String::from("import.sh"))
                    .await;
                
                event!(Level::TRACE, "Unpacked script");

                if let (Some(import), Some(user), Some(token), Some(registry_name), Some(registry_host), Some(repo), Some(reference)) = (
                    tc.search().find_symbol("import"),
                    tc.search().find_text("user"),
                    tc.search().find_text("token"),
                    tc.search().find_symbol("registry_name"),
                    tc.search().find_symbol("registry_host"),
                    tc.search().find_symbol("repo"),
                    tc.search().find_symbol("reference")
                ) {
                    event!(Level::DEBUG, "Preparing a registry-env for import process");
                    tc.state_mut()
                        .with_symbol("process", "sh import.sh")
                        .with_symbol("env", "REGISTRY_NAME")
                        .with_symbol("env", "REGISTRY_HOST")
                        .with_symbol("env", "REGISTRY_USER")
                        .with_symbol("env", "REGISTRY_TOKEN")
                        .with_symbol("env", "REPO")
                        .with_symbol("env", "REFERENCE")
                        .with_symbol("env", "SOURCE")
                        .with_symbol("REGISTRY_NAME", &registry_name)
                        .with_symbol("REGISTRY_HOST", &registry_host)
                        .with_symbol("REGISTRY_USER", &user)
                        .with_symbol("REGISTRY_TOKEN", &token)
                        .with_symbol("REPO", &repo)
                        .with_symbol("REFERENCE", &reference);

                        if let Some(platform) = tc.search().find_symbol("platform") {
                            if platform != "all" {
                                // 1) resolve the manifest list
                                if let Some(client) = tc.client() {
                                    
                                    if let Some((ns, reference)) = import.split_once(":") {
                                        if let Some((host, repo)) = ns.split_once("/") {
                                            let manifest_uri = format!("{host}/v2/{repo}/{reference}");
                                            event!(Level::DEBUG, "Checking to see if {manifest_uri} is a manifest list"); 
    
                                            let req = Request::builder()
                                                .uri_str(manifest_uri)
                                                .header("accept", "application/vnd.docker.distribution.manifest.list.v2+json")
                                                .finish();
                                            
                                            if let Some(resp) = client.request(req.into()).await.ok() {
                                                event!(Level::DEBUG, "Received response, checking");
    
                                                if let Some((_os, _arch)) = platform.split_once("/") {
                                                    match hyper::body::to_bytes(resp.into_body()).await {
                                                        Ok(bytes) => {
                                                            if let Some(manifest_list) = serde_json::from_slice::<ImageIndex>(&bytes).ok() {
                                                                if let Some(desc) = manifest_list.manifests.iter().find(|d| match &d.platform {
                                                                    Some(Platform{ 
                                                                        os,
                                                                        architecture,
                                                                        ..
                                                                    }) if os == _os && architecture == _arch => {
                                                                        true
                                                                    }
                                                                    _ => false,
                                                                }) {
                                                                    let true_source = format!("{host}/{repo}@{}", desc.digest);
                                                                    event!(Level::DEBUG, "Found true source {true_source}");
                                                                    tc.state_mut().with_symbol("SOURCE", &true_source);
                                                                }
                                                            } 
                                                        },
                                                        Err(err) => {
                                                            event!(Level::ERROR, "Could not read body {err}");
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                tc.state_mut().with_symbol("SOURCE", &import);
                            }
                        } else {
                            tc.state_mut().with_symbol("SOURCE", &import);
                        }

                        let (task, cancel) = Process::call(&tc).expect("Should start");
                        select! {
                            tc = task => {
                                if let Some(mut tc) = tc.ok() {
                                    event!(Level::DEBUG, "Finished importing - {import} -> {registry_name}.{registry_host}/{repo}:{reference}");
                                    tc.copy_previous();
                                    return Some(tc);
                                } else {
                                    return None;
                                }
                            }
                            _ = cancel_source => {
                                cancel.send(()).ok();
                                return None;
                            }
                        }
                }

                tc.copy_previous();
                Some(tc)
            }
        })
    }

    fn compile(parser: &mut AttributeParser) {
        parser.add_custom_with("platform", |p, content|{ 
            if let Some(last_child_entity) = p.last_child_entity() {
                p.define_child(last_child_entity, "platform", Value::Symbol(content))
            }
        })
    }
}


impl BlockObject for Import {
    fn query(&self) -> BlockProperties {
        BlockProperties::default()
            .require("import")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}

