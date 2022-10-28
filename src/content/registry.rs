use std::collections::HashMap;
use std::sync::Arc;

use crate::{ArtifactManifest, ImageIndex, ImageManifest, ReferrersList};
use hyper::{Body, StatusCode};
use lifec::engine::Engines;
use lifec::prelude::{Appendix, Entities, Events, Thunk, ThunkContext, Value};
use lifec::project::Workspace;
use lifec::state::AttributeIndex;
use lifec_poem::RoutePlugin;
use poem::{Request, Response};
use specs::{prelude::*, SystemData};
use tracing::{event, Level};

use super::Contents;

/// System data for registry components,
///
#[derive(SystemData)]
pub struct Registry<'a> {
    workspace: Read<'a, Option<Workspace>>,
    entity_index: Read<'a, HashMap<String, Entity>>,
    events: Events<'a>,
    // contents: Contents<'a>,
    // artifacts: WriteStorage<'a, ArtifactManifest>,
    // referrers: WriteStorage<'a, ReferrersList>,
    // indexes: WriteStorage<'a, ImageIndex>,
    // images: WriteStorage<'a, ImageManifest>,
}

impl<'a> Registry<'a> {
    /// Takes a request and a route_plugin and handles proxying the response,
    ///
    pub fn proxy_request<P>(
        &mut self,
        operation_name: impl AsRef<str>,
        request: &Request,
        body: Option<Body>,
        namespace: impl AsRef<str>,
        repo: impl AsRef<str>,
        reference: impl AsRef<str>,
    ) -> Response
    where
        P: RoutePlugin,
    {
        let operation_name = operation_name.as_ref();
        let operation_name = if let Some(tag) = self
            .workspace
            .as_ref()
            .expect("should have a workspace")
            .tag()
        {
            format!("adhoc-{operation_name}#{tag}")
        } else {
            format!("adhoc-{operation_name}")
        };

        let operation = self
            .entity_index
            .get(&operation_name)
            .expect("should have an operation entity");
        let spawned = self.events.spawn(*operation);

        let context = self.prepare_registry_context(request, namespace, repo, reference, spawned);

        self.events.start(spawned, Some(&context));

        tokio::task::block_in_place(|| {
            if let Some(mut result) = self.events.wait_on(spawned) {
                if let Some(body) = body {
                    result.cache_body(body);
                }
    
                P::response(&mut result)
            } else {
                Self::soft_fail()
            }
        })
    }

    /// Fails in a way that the runtime will fallback to the upstream server
    pub fn soft_fail() -> Response {
        Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish()
    }

    /// Returns a context prepared with registry context,
    ///
    pub fn prepare_registry_context(
        &self,
        request: &Request,
        namespace: impl AsRef<str>,
        repo: impl AsRef<str>,
        reference: impl AsRef<str>,
        entity: Entity,
    ) -> ThunkContext {
        let headers = request.headers();

        let mut context = self.events.plugins().initialize_context(entity, None);
        let workspace = context
            .workspace()
            .expect("should have a workspace")
            .clone();

        let graph = context.modify_graph();
        graph.add_control(
            "REGISTRY_NAMESPACE",
            Value::Symbol(namespace.as_ref().to_string()),
        );
        graph.add_control("REGISTRY_REPO", Value::Symbol(repo.as_ref().to_string()));
        graph.add_control("REFERENCE", Value::Symbol(reference.as_ref().to_string()));
        graph.add_control(
            "REGISTRY_HOST",
            Value::Symbol(workspace.get_host().to_string()),
        );
        graph.add_control(
            "REGISTRY_TENANT",
            Value::Symbol(
                workspace
                    .get_tenant()
                    .expect("should have a tenant")
                    .to_string(),
            ),
        );

        for (name, value) in headers {
            context
                .state_mut()
                .with_symbol("header", name.to_string())
                .with_symbol(
                    "method",
                    request.method().as_str().to_string().to_uppercase(),
                )
                .with_symbol(
                    name.to_string(),
                    value.to_str().expect("should be a string").to_string(),
                );
        }

        context
    }
}
