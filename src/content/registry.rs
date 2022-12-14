use hyper::{Body, StatusCode, Uri};
use lifec::engine::NodeCommand;
use lifec::prelude::{SpecialAttribute, ThunkContext};
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
        operation_name: impl Into<String>,
        request: &Request,
        body: Option<Body>,
        namespace: impl Into<String>,
        repo: impl Into<String>,
        reference: impl Into<String>,
    ) -> Response
    where
        P: RoutePlugin + SpecialAttribute,
    {
        let operation_name = operation_name.into();
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

        let context =
            self.prepare_registry_context::<P>(request, namespace, repo, reference, context);

        if let Some(yielding) = context.dispatch_node_command(NodeCommand::Spawn(*operation)) {
            match yielding.await {
                Ok(mut context) => {
                    if let Some(body) = body {
                        context.cache_body(body);
                    }

                    let response = P::response(&mut context);

                    if response.status().is_redirection() {
                        if let Some(api) = response
                            .headers()
                            .get("location")
                            .and_then(|api| api.to_str().ok())
                            .and_then(|api| api.parse::<Uri>().ok())
                        {
                            event!(Level::DEBUG, "Handling redirect, {api}");
                            let client = context.client().expect("should have client");
                            match client.get(api).await {
                                Ok(resp) => resp.into(),
                                Err(err) => panic!("error following redirect {err}"),
                            }
                        } else {
                            event!(Level::DEBUG, "No location header");
                            response.into()
                        }
                    } else {
                        response
                    }
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
        namespace: impl Into<String>,
        repo: impl Into<String>,
        reference: impl Into<String>,
        context: &ThunkContext,
    ) -> ThunkContext
    where
        S: SpecialAttribute,
    {
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
        let repo = repo.into();
        let resource = S::ident();
        let reference = reference.into();
        let namespace = namespace.into();

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
            .with_symbol(
                "api",
                format!("https://{namespace}/v2/{repo}/{resource}/{reference}"),
            );
            
        let headers = request.headers();
        for (name, value) in headers
            .iter()
            .filter(|(n, _)| n.as_str() != "host" && n.as_str() != "user-agent")
        {
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
