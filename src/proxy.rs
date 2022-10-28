use hyper::http::StatusCode;
use lifec::{
    prelude::{
        AttributeParser, Block, BlockIndex, Host, Parser, Run, SpecialAttribute,
        ThunkContext, Value, World,
    },
    project::{default_parser, default_runtime, Operations, Project, default_world},
    runtime::Runtime,
    state::{AttributeGraph, AttributeIndex},
};
use lifec_poem::{WebApp, RoutePlugin};
use logos::Logos;
use poem::{
    get, handler, post, put,
    web::{Data, Query},
    EndpointExt, Request, Response, Route,
};
use serde::Deserialize;
use specs::{WorldExt, Join};
use std::{collections::HashMap, sync::Arc};
use tracing::{event, Level};

use crate::{
    Artifact, Authenticate, Continue, Discover, FormatOverlayBD, Import, Login, LoginACR,
    LoginOverlayBD, Mirror, Resolve, Teleport,
};

mod proxy_target;
pub use proxy_target::ProxyTarget;

mod methods;
use methods::Methods;

mod resources;
use resources::Resources;

mod blobs;
use blobs::blob_chunk_upload_api;
use blobs::blob_download_api;
use blobs::blob_upload_api;

mod manifests;
use manifests::manifests_api;

mod tags;
use tags::tags_api;

mod resolve;
pub use resolve::Manifests;

/// Struct for creating a customizable registry proxy,
///
/// # Customizable registry proxy
///
/// This special attribute enables describing a customizable registry proxy.
/// Underneath the hood, this enables the `.runtime` attribute so that plugin
/// declarations can be assigned an entity/event. Next this attribute adds 3
/// custom attributes `manifests`, `blobs`, `tags` which represent the 3 core
/// resources the OCI distribution api hosts. Methods for each resource can be
/// customized with a sequence of plugin calls. Since these calls aren't part
/// of the normal event runtime flow, they are executed with the host.execute(..)
/// extension method instead.
///
/// ## Example proxy definition
///
/// ```md
/// <``` start proxy>
/// # Proxy setup
/// + .proxy                  localhost:8567
/// ## Resolve manifests and artifacts
/// - This example shows how the proxy can be configuired to make discover calls,
/// - When an image manifest is being resolved, the proxy will also call discover on artifacts
///
/// : .manifests head, get
/// :   .login                  access_token
/// :   .authn                  oauth2
/// :   .resolve                application/vnd.oci.image.manifest.v1+json
/// :   .discover               dadi.image.v1
/// :   .discover               notary.signature.v1
///
/// ## Teleport and dispatch a convert operation if teleport isn't available
/// :   .teleport               overlaybd
/// :   .converter              convert overlaybd
///
/// ## Validate signatures, or create a signature if it doesn't exist
/// :   .notary
/// :   .reject_if_missing
/// :   .sign_if_missing
///
/// ## Download blobs
/// : .blobs head, get
/// : .login                  login.pfx
/// : .authn                  cert
/// : .pull
/// <```>  
/// ```
#[derive(Default)]
pub struct RegistryProxy {
    host: Arc<Host>,
}

impl RegistryProxy {
    /// Fails in a way that the runtime will fallback to the upstream server
    pub fn soft_fail() -> Response {
        Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish()
    }
}

impl SpecialAttribute for RegistryProxy {
    fn ident() -> &'static str {
        "proxy"
    }

    /// This alias is so that `.proxy` stable attributes are not interpreted
    /// by the normal `.engine` interpreter. However, we still want access to the world's runtime
    /// on `parse()`
    ///
    fn parse(parser: &mut AttributeParser, content: impl AsRef<str>) {
        parser.define("app_host", Value::Symbol(content.as_ref().to_string()));

        parser.with_custom::<Manifests>();
    }
}

impl Project for RegistryProxy {
    fn interpret(_: &World, _: &Block) {}

    fn parser() -> Parser {
        default_parser(Self::world()).with_special_attr::<RegistryProxy>()
    }

    fn runtime() -> Runtime {
        let mut runtime = default_runtime();
        runtime.install_with_custom::<Run<RegistryProxy>>("");
        runtime.install_with_custom::<LoginACR>("");
        runtime.install_with_custom::<Authenticate>("");
        runtime.install_with_custom::<Mirror>("");
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Resolve>("");
        runtime.install_with_custom::<Discover>("");
        runtime.install_with_custom::<Teleport>("");
        runtime.install_with_custom::<Continue>("");
        runtime.install_with_custom::<Artifact>("");
        runtime.install_with_custom::<Import>("");
        runtime.install_with_custom::<LoginOverlayBD>("");
        runtime.install_with_custom::<FormatOverlayBD>("");
        runtime
    }

    fn world() -> World {
        let mut world = default_world();
        world.insert(Self::runtime());
        world.register::<Manifests>();
        world
    }
}

impl WebApp for RegistryProxy {
    fn create(context: &mut ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
        let mut route = Route::default();

        for manifest in self.host.world().read_component::<Manifests>().join() {
            if manifest.can_route() {
                let mut manifest = manifest.clone();
                manifest.set_host(self.host.clone());


                route = route.nest("/v2", manifest.route());
            }
        }

        route
    }
}

#[derive(Deserialize)]
struct IndexParams {
    ns: Option<String>,
}
#[handler]
async fn index(
    request: &Request,
    Query(IndexParams { ns }): Query<IndexParams>,
    context: Data<&ThunkContext>,
    host: Data<&Host>,
) -> Response {
    event!(Level::DEBUG, "Got /v2 request");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = context.clone();

    if let Some(ns) = ns {
        input.state_mut().with_symbol("ns", &ns);
    }

    // TODO Dump proxy state here
    todo!()
}

impl From<ThunkContext> for RegistryProxy {
    fn from(context: ThunkContext) -> Self {
        let workspace = context.workspace().expect("should have a work_dir");
        let registry_host = workspace.get_host().to_string();
        let registry_tenant = workspace
            .get_tenant()
            .expect("should have tenant")
            .to_string();
        
        let mut host = Host::load_workspace::<RegistryProxy>(
            None,
            &registry_host,
            &registry_tenant,
            None::<String>,
            context.tag(),
        );
        host.prepare::<RegistryProxy>();

        Self {
            host: Arc::new(host),
        }
    }
}

impl RegistryProxy {
    /// Handles executing the proxy sequence
    ///
    pub async fn handle(
        host: &Host,
        resource: impl AsRef<str>,
        method: impl AsRef<str>,
        input: &ThunkContext,
    ) -> Response {
        let resource = resource.as_ref();
        let method = method.as_ref();

        let mut operations = host.world().system_data::<Operations>();
        let mut operation = operations
            .execute_operation(
                format!("{resource}.{method}"),
                input.find_symbol("tag"),
                Some(input),
            )
            .expect("should have started an operation");

        let (_, rx) = tokio::sync::oneshot::channel();

        match operation.task(rx).await {
            Some(result) => RegistryProxy::into_response(&result),
            None => {
                event!(Level::ERROR, "Error handling call sequence");
                RegistryProxy::soft_fail()
            }
        }
    }

    pub fn into_response(context: &ThunkContext) -> Response {
        if let (Some(location), Some(301 | 307 | 308)) = (
            context.find_symbol("location"),
            context.find_int("status_code"),
        ) {
            let content_type = context
                .search()
                .find_symbol("content-type")
                .expect("A content type should've been provided");
            let digest = context
                .search()
                .find_symbol("digest")
                .expect("A digest should've been provided");

            Response::builder()
                .status(StatusCode::MOVED_PERMANENTLY)
                .header("location", location)
                .header("docker-content-digest", digest)
                .header("content-type", content_type)
                .finish()
        } else if let Some(body) = context.state().find_binary("body") {
            let content_type = context
                .search()
                .find_symbol("content-type")
                .expect("A content type should've been provided");
            let digest = context
                .search()
                .find_symbol("digest")
                .expect("A digest should've been provided");

            let mut response = Response::builder()
                .status(StatusCode::OK)
                .content_type(content_type)
                .header("Docker-Content-Digest", digest);

            if let Some(location) = context.search().find_symbol("location") {
                response = response.header("Location", location);
            }

            response.body(body)
        } else {
            Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .finish()
        }
    }

    /// Extracts route definitions for the proxy and calls on_route for each route found,
    ///
    pub fn extract_routes(block_index: &BlockIndex) -> Vec<AttributeGraph> {
        let mut graphs = vec![];
        let graph = AttributeGraph::new(block_index.clone());

        if let Some(proxy_entity) = graph.find_int("proxy_entity") {
            let original = graph.entity_id();
            let proxy_entity = graph
                .scope(proxy_entity as u32)
                .expect("proxy entity should have been placed in the child properties");

            for route_value in proxy_entity.find_values("route") {
                match route_value {
                    Value::Int(id) if id as u32 != original => {
                        let graph = graph.scope(id as u32).expect("should be a route");
                        graphs.push(graph);
                    }
                    _ => continue,
                }
            }

            let graph = graph.unscope();
            graphs.push(graph);
        }

        graphs
    }
}

mod tests {
    #[test]
    #[tracing_test::traced_test]
    fn test_proxy_parsing() {
        use crate::RegistryProxy;
        use hyper::Client;
        use hyper::StatusCode;
        use hyper_tls::HttpsConnector;
        use lifec::prelude::*;
        use lifec_poem::WebApp;

        let mut host = Host::load_content::<RegistryProxy>(
            r#"
        # Example proxy definition
        ``` start proxy
        # Proxy setup
        + .proxy                  localhost:8567

        : .manifests head, get
        : .println test
        : .println that
        : .println manifests
        : .println sequence
        : .println works

        : .blobs head, get
        : .println test
        : .println that
        : .println blobs
        : .println sequence
        : .println works
        ```
        "#,
        );

        let mut dispatcher = Host::dispatcher_builder().build();

        dispatcher.setup(host.world_mut());

        let block = Engine::find_block(host.world(), "start proxy").expect("block is created");
        let block = {
            let blocks = host.world().read_component::<Block>();
            blocks.get(block).unwrap().clone()
        };

        let index = block.index().first().expect("should exist").clone();
        let graph = AttributeGraph::new(index);

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let world = World::new();
            let entity = world.entities().create();
            let https = HttpsConnector::new();
            let client = Client::builder().build::<_, hyper::Body>(https);
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let handle = runtime.handle();

            let mut tc = ThunkContext::default()
                .with_block(&block)
                .with_state(graph)
                .enable_https_client(client)
                .enable_async(entity, handle.clone());

            let app = RegistryProxy::create(&mut tc).routes();
            let cli = poem::test::TestClient::new(app);

            let resp = cli
                .get("/v2/library/test/manifests/test_ref?ns=test.com")
                .send()
                .await;
            resp.assert_status(StatusCode::SERVICE_UNAVAILABLE);

            let resp = cli
                .get("/v2/library/test/blobs/test_digest?ns=test.com")
                .send()
                .await;
            resp.assert_status(StatusCode::SERVICE_UNAVAILABLE);

            // TODO add these tests back
            // let resp = cli.get("/").send().await;
            // resp.assert_status(StatusCode::NOT_FOUND);

            // let resp = cli.head("/").send().await;
            // resp.assert_status(StatusCode::NOT_FOUND);

            // let resp = cli.get("/v2").send().await;
            // resp.assert_status_is_ok();

            // let resp = cli.get("/v2/").send().await;
            // resp.assert_status_is_ok();

            // let resp = cli.head("/v2").send().await;
            // resp.assert_status_is_ok();

            // let resp = cli.head("/v2/").send().await;
            // resp.assert_status_is_ok();

            // let resp = cli
            //     .head("/v2/library/test/manifests/test_ref?ns=test.com")
            //     .send()
            //     .await;
            // resp.assert_status_is_ok();

            // let resp = cli
            //     .put("/v2/library/test/manifests/test_ref?ns=test.com")
            //     .send()
            //     .await;
            // resp.assert_status_is_ok();

            // let resp = cli
            //     .delete("/v2/library/test/manifests/test_ref?ns=test.com")
            //     .send()
            //     .await;
            // resp.assert_status_is_ok();

            // let resp = cli
            //     .post("/v2/library/test/blobs/uploads?ns=test.com")
            //     .send()
            //     .await;
            // resp.assert_status_is_ok();

            // let resp = cli
            //     .patch("/v2/library/test/blobs/uploads/test?ns=test.com")
            //     .send()
            //     .await;
            // resp.assert_status_is_ok();

            // let resp = cli
            //     .put("/v2/library/test/blobs/uploads/test?ns=test.com")
            //     .send()
            //     .await;
            // resp.assert_status_is_ok();

            // let resp = cli
            //     .get("/v2/library/test/tags/list?ns=test.com")
            //     .send()
            //     .await;
            // resp.assert_status_is_ok();

            runtime.shutdown_background();
        });
    }
}
