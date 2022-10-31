use hyper::{Body, StatusCode};
use lifec::prelude::{NodeCommand, SpecialAttribute, ThunkContext};
use lifec::project::Workspace;
use lifec::state::AttributeIndex;
use lifec_poem::RoutePlugin;
use poem::{Request, Response};
use specs::{prelude::*, SystemData};
use std::collections::HashMap;
use tracing::{event, Level};

/// System data for registry components,
///
#[derive(SystemData)]
pub struct Registry<'a> {
    workspace: Read<'a, Option<Workspace>>,
    entity_index: Read<'a, HashMap<String, Entity>>,
    // contents: Contents<'a>,
    // artifacts: WriteStorage<'a, ArtifactManifest>,
    // referrers: WriteStorage<'a, ReferrersList>,
    // indexes: WriteStorage<'a, ImageIndex>,
    // images: WriteStorage<'a, ImageManifest>,
}

impl<'a> Registry<'a> {
    /// Takes a request and a route_plugin and handles proxying the response,
    ///
    pub async fn proxy_request<P>(
        &mut self,
        context: &ThunkContext,
        operation_name: impl AsRef<str>,
        request: &Request,
        body: Option<Body>,
        namespace: impl AsRef<str>,
        repo: impl AsRef<str>,
        reference: impl AsRef<str>,
    ) -> Response
    where
        P: RoutePlugin + SpecialAttribute,
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

        let context = self.prepare_registry_context::<P>(request, namespace, repo, reference, context);

        if let Some(yielding) = context.dispatch_node_command(NodeCommand::Spawn(*operation))
        {
            match yielding.await {
                Ok(mut context) => {
                    if let Some(body) = body {
                        context.cache_body(body);
                    }

                    P::response(&mut context)
                }
                Err(err) => {
                    event!(
                        Level::ERROR,
                        "Could not receive result from yielding channel, {err}"
                    );
                    Self::soft_fail()
                }
            }
        } else {
            Self::soft_fail()
        }
    }

    /// Fails in a way that the runtime will fallback to the upstream server
    pub fn soft_fail() -> Response {
        Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish()
    }

    /// Returns a context prepared with registry context,
    ///
    pub fn prepare_registry_context<S>(
        &self,
        request: &Request,
        namespace: impl AsRef<str>,
        repo: impl AsRef<str>,
        reference: impl AsRef<str>,
        context: &ThunkContext,
    ) -> ThunkContext
    where
        S: SpecialAttribute,
    {
        let headers = request.headers();
        let mut context = context.clone();
        let workspace = context
            .workspace()
            .expect("should have a workspace")
            .clone();
        let tenant = workspace
            .get_tenant()
            .expect("should have a tenant")
            .to_string();

        let host = workspace.get_host().to_string();
        let repo = repo.as_ref().to_string();
        let resource = S::ident();
        let reference = reference.as_ref().to_string();
        let namespace = namespace.as_ref().to_string();

        context
            .with_symbol("REGISTRY_NAMESPACE", &namespace)
            .with_symbol("REGISTRY_REPO", &repo)
            .with_symbol("REFERENCE", &reference)
            .with_symbol("REGISTRY_HOST", host)
            .with_symbol("REGISTRY_TENANT", tenant)
            .with_symbol(
                "method",
                request.method().as_str().to_string().to_uppercase(),
            )
            .with_symbol(
                "WORK_DIR",
                workspace.work_dir().to_str().expect("should be a string"),
            )
            .with_symbol("api", format!("https://{namespace}/v2/{repo}/{resource}/{reference}"));

        for (name, value) in headers {
            context
                .state_mut()
                .with_symbol("header", name.to_string())
                .with_symbol(
                    name.to_string(),
                    value.to_str().expect("should be a string").to_string(),
                );
        }

        context.commit()
    }
}
