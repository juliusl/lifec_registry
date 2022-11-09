use std::sync::Arc;

use hyper::{body::HttpBody, Body, Response};
use imgui::{Ui, Window};
use lifec::{
    debugger::Debugger,
    guest::Guest,
    prelude::{
        Appendix, BlockObject, BlockProperties, Editor, Host, Node, Plugin, Sequencer, ThunkContext,
    },
};
use serde::Deserialize;
use sha2::Digest;
use specs::{Component, WorldExt};
use tracing::{event, Level};

use crate::{
    content::{DOCKER_MANIFEST_LIST, DOCKER_V1_MANIFEST, DOCKER_V2_MANIFEST, OCI_IMAGE_MANIFEST},
    ArtifactManifest, Descriptor, ImageIndex, ImageManifest, RegistryProxy,
    OCI_ARTIFACTS_MANIFEST_MEDIA_TYPE, ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE,
};

/// Plugin to store registry content locally,
///
#[derive(Default)]
pub struct Store;

impl Store {
    /// Read content,
    ///
    pub async fn read_content<T>(response: Response<Body>) -> Option<(Descriptor, T)>
    where
        T: for<'a> Deserialize<'a>,
    {
        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.parse::<usize>().ok());

        let docker_content_digest = response
            .headers()
            .get("docker-content-digest")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| Some(h.to_string()));

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| Some(h.to_string()));

        let body = response.into_body();
        if let Some(upper) = body.size_hint().upper() {
            if let Some(content_length) = content_length {
                assert!(
                    upper <= content_length as u64,
                    "Stream size is larger then content length header"
                );
            }
        }

        match hyper::body::to_bytes(body).await {
            Ok(bytes) => {
                let bytes = bytes.as_ref().to_vec();

                if let Some(content_digest) = docker_content_digest.as_ref() {
                    if content_digest.starts_with("sha256") {
                        let mut digest = sha2::Sha256::new();
                        digest.update(&bytes);
                        let content_digest = content_digest.trim_start_matches("sha256:");
                        let computed = format!("{:02x?}", digest.finalize());
                        let computed = computed
                            .replace('[', "")
                            .trim_end_matches(']')
                            .split(", ")
                            .collect::<Vec<_>>()
                            .join("");
                        assert_eq!(computed, content_digest);
                    } else if content_digest.starts_with("sha512") {
                        let mut digest = sha2::Sha512::new();
                        digest.update(&bytes);
                        let content_digest = content_digest.trim_start_matches("sha512:");
                        let computed = format!("{:02x?}", digest.finalize())
                            .replace('[', "")
                            .trim_end_matches(']')
                            .split(", ")
                            .collect::<Vec<_>>()
                            .join("");
                        assert_eq!(computed, content_digest);
                    } else {
                        panic!("Unrecognized content_digest");
                    }
                }

                if let Some(obj) = serde_json::from_slice::<T>(&bytes).ok() {
                    Some((
                        Descriptor {
                            media_type: content_type.expect("should have a content type"),
                            digest: docker_content_digest.expect("should have a digest"),
                            size: content_length.expect("should have a content length") as u64,
                            ..Default::default()
                        },
                        obj,
                    ))
                } else {
                    None
                }
            }
            Err(err) => {
                event!(Level::ERROR, "Could not read body, {err}");
                None
            }
        }
    }
}

impl Plugin for Store {
    fn symbol() -> &'static str {
        "store"
    }

    fn description() -> &'static str {
        "Stores registry content locally"
    }

    fn call(context: &mut lifec::prelude::ThunkContext) -> Option<lifec::prelude::AsyncContext> {
        // If there's a response cached,
        let response = context.take_response();

        context.task(|_| {
            let tc = context.clone();
            async {
                //
                if let Some(response) = response {
                    match response
                        .headers()
                        .get("content-type")
                        .and_then(|h| h.to_str().ok())
                    {
                        Some(ORAS_ARTIFACTS_MANIFEST_MEDIA_TYPE)
                        | Some(OCI_ARTIFACTS_MANIFEST_MEDIA_TYPE) => {
                            if let Some((desc, manifest)) =
                                Store::read_content::<ArtifactManifest>(response).await
                            {
                            }
                        }
                        Some(DOCKER_V1_MANIFEST)
                        | Some(DOCKER_V2_MANIFEST)
                        | Some(OCI_IMAGE_MANIFEST) => {
                            if let Some((desc, manifest)) =
                                Store::read_content::<ImageManifest>(response).await
                            {
                                t(tc.clone(), desc, manifest);
                            }
                        }
                        Some(DOCKER_MANIFEST_LIST) => {
                            let manifest = Store::read_content::<ImageIndex>(response).await;
                            eprintln!("{:#?}", manifest);
                        }
                        _ => {}
                    }
                }

                Some(tc)
            }
        })
    }
}

fn t<T>(tc: ThunkContext, desc: Descriptor, comp: T)
where
    T: Component,
{
    let world = tc
        .workspace()
        .expect("should have a workspace")
        .compile::<RegistryProxy>()
        .expect("should compile into a world");
    let mut host = Host::from(world);
    host.prepare::<RegistryProxy>();
    host.link_sequences();
    host.build_appendix();
    host.enable_listener::<Debugger>();
    host.prepare::<RegistryProxy>();
    let appendix = host
        .as_mut()
        .remove::<Appendix>()
        .expect("should be able to remove appendix");
    let appendix = Arc::new(appendix);
    host.world_mut().insert(appendix.clone());
    let entity = tc.entity().expect("should have an entity");
    let guest_entity = host.world().entities().entity(entity.id());

    host.world()
        .write_component()
        .insert(guest_entity, comp)
        .expect("should be able to insert manifest");

    host.world()
        .write_component()
        .insert(guest_entity, desc)
        .expect("should be able to insert");

    let mut guest = Guest::new::<RegistryProxy>(entity, host, |g| {});

    guest.add_node(Node {
        appendix,
        remote_protocol: Some(guest.subscribe()),
        status: lifec::prelude::NodeStatus::Custom(entity),
        edit: Some(|n, ui| {
            let opened = true;
            Window::new("Registry Object").build(ui, || {
                if let Some(r) = n.remote_protocol.as_ref() {
                    if let Some(desc) = r
                        .remote
                        .borrow()
                        .as_ref()
                        .read_component::<Descriptor>()
                        .get(n.status.entity())
                    {
                        ui.input_text("digest", &mut desc.digest.clone())
                            .read_only(true)
                            .build();
                        ui.input_text("size (bytes)", &mut format!("{}", &desc.size))
                            .read_only(true)
                            .build();
                        ui.input_text("media_type", &mut desc.media_type.clone())
                            .read_only(true)
                            .build();
                        if let Some(artifact_type) = desc.artifact_type.as_ref() {
                            ui.input_text("artifact_type", &mut artifact_type.clone())
                                .read_only(true)
                                .build();
                        }
                    }
                }
            });
            opened
        }),
        ..Default::default()
    });
    guest.enable_remote();

    if tc.enable_guest(guest) {}
}

impl BlockObject for Store {
    fn query(&self) -> lifec::prelude::BlockProperties {
        BlockProperties::default().optional("store")
    }

    fn parser(&self) -> Option<lifec::prelude::CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}