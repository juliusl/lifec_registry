use hyper::{Body, StatusCode, Uri};
use lifec::engine::NodeCommand;
use lifec::prelude::{SpecialAttribute, ThunkContext};
use lifec::state::AttributeIndex;
use lifec_poem::RoutePlugin;
use poem::{Request, Response};
use tracing::{debug, event, Level, error, info};

use crate::hosts_config::MirrorHost;

pub mod consts {
    /// While an image is being resolved, if the registry is capable of streaming the image then including this header will
    /// allow the mirror to check if the registry is currently storing the streamable format of the image. The value of this header,
    /// should be the desired streamable format.
    ///
    pub const UPGRADE_IF_STREAMABLE_HEADER: &'static str = "x-ms-upgrade-if-streamable";

    /// While an image is being resolved by the default mirror host, if this header is included it allows the mirror to check the suffix of the
    /// request uri in order to enabled a dedicated hosts config for the incoming host. The value of this header should be the suffix to check.
    ///
    pub const ENABLE_MIRROR_IF_SUFFIX_HEADER: &'static str = "x-ms-enable-mirror-if-suffix";

    /// While an image is being resolved, if this header is included it allows the mirror to check the suffix of the request uri in order to
    /// determine of the mirror should accept this request.
    ///
    pub const ACCEPT_IF_SUFFIX_HEADER: &'static str = "x-ms-accept-if-suffix";
}

/// Pointer struct for fn implementations,
///
#[derive(Default, Clone)]
pub struct Registry;

impl Registry {
    /// Takes a request and a route_plugin and handles proxying the response,
    ///
    pub async fn proxy_request<P>(
        &self,
        context: &ThunkContext,
        operation_name: impl Into<String>,
        request: &Request,
        body: Option<Body>,
        namespace: impl Into<String>,
        repo: impl Into<String>,
        reference: Option<impl Into<String>>,
    ) -> Response
    where
        P: RoutePlugin + SpecialAttribute,
    {
        let mut repo = repo.into();
        let mut namespace = namespace.into();

        if repo.starts_with("_tenant_") {
            if let Some((tenant, _repo)) = repo.split_once("/") {
                let tenant = tenant.trim_start_matches("_tenant_");
                namespace = format!("{tenant}.{namespace}");
                repo = _repo.to_string();
                info!("Applied tenant workaround, namespace -> {namespace}, repo -> {repo}");
            }
        }

        // Check if the request uri ends with the suffix value of the header, if not then return 503 Service Unavailable
        let accept_if_header = request.header(consts::ACCEPT_IF_SUFFIX_HEADER);
        if accept_if_header.is_some() && !accept_if_header
            .filter(|f| f.len() < 256)
            .map(|suffix| namespace.ends_with(suffix))
            .unwrap_or_default()
        {
            debug!("Rejecting host {:?}", namespace);
            return Self::soft_fail();
        }

        // Check if the request uri ends with the suffix value of the header, if so enable a mirror hosts config for the request host
        if request
            .header(consts::ENABLE_MIRROR_IF_SUFFIX_HEADER)
            .filter(|f| f.len() < 256)
            .map(|suffix| namespace.ends_with(suffix))
            .unwrap_or_default()
        {
            let mirror_hosts_config = MirrorHost::get_hosts_config(
                &namespace,
                "http://localhost:8578",
                true,
                Some("overlaybd"),
            );

            if let Err(err) = mirror_hosts_config.install(None::<String>) {
                error!("Unable to enable mirror host config for, {}, {:?}", namespace, err);
            } else {
                debug!("Enabled mirror host config for {}", namespace);
            }
        }

        let workspace = context
            .workspace()
            .map(|w| {
                if let Some(format) = request
                    .header(crate::consts::UPGRADE_IF_STREAMABLE_HEADER)
                    .filter(|f| f.len() < 256)
                {
                    w.use_tag(format)
                } else {
                    w.to_owned()
                }
            })
            .expect("should return a workspace by this point");

        let operation_name = operation_name.into();
        let operation = workspace.find_operation(&operation_name).expect(&format!(
            "should have an operation entity for {}",
            &operation_name
        ));

        debug!(
            "Found operation {}, entity: {:?}, tag: {:?}",
            operation_name,
            &operation,
            workspace.tag()
        );

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
        reference: Option<impl Into<String>>,
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
            .map(|t| t.to_string())
            .unwrap_or_default();

        let host = workspace.get_host().to_string();
        let repo = repo.into();
        let resource = S::ident();
        let reference = reference.map(|r| r.into()).unwrap_or_default();
        let namespace = namespace.into();

        debug!(
            "Preparing proxy context - host: {}, namespace: {} repo: {}, reference: {}",
            &host, &namespace, &repo, &reference
        );

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
