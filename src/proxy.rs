use hyper::http::StatusCode;
use lifec::{
    default_parser, default_runtime, AttributeGraph, AttributeIndex, BlockIndex, CustomAttribute,
    Executor, Host, Project, Runtime, SpecialAttribute, Start, ThunkContext, Value,
};
use lifec_poem::WebApp;
use logos::Logos;
use poem::{
    get, handler, post, put,
    web::{Data, Query},
    EndpointExt, Request, Response, Route,
};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use tracing::{event, Level};

use crate::{
    Artifact, Authenticate, Continue, Discover, FormatOverlayBD, Login, LoginACR, LoginOverlayBD,
    Mirror, Resolve, Teleport,
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
pub struct Proxy {
    context: ThunkContext,
    host: Arc<Host>,
}

impl Proxy {
    /// Fails in a way that the runtime will fallback to the upstream server
    pub fn soft_fail() -> Response {
        Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .finish()
    }
}

impl SpecialAttribute for Proxy {
    fn ident() -> &'static str {
        "proxy"
    }

    /// This alias is so that `.proxy` stable attributes are not interpreted
    /// by the normal `.engine` interpreter. However, we still want access to the world's runtime
    /// on `parse()`
    ///
    fn parse(parser: &mut lifec::AttributeParser, content: impl AsRef<str>) {
        parser.define("app_host", Value::Symbol(content.as_ref().to_string()));

        Runtime::parse(parser, &content);

        // A new entity is created per resource/method being proxied
        // When the below attributes are parsed, the context will be set to that entity
        // So that subsequent plugin definitions will modify the "sequence" property
        // of the proxied resource. This is how a call sequence can be built per resource
        // without modifying external engine/runtime in the host.
        parser.add_custom(CustomAttribute::new_with("manifests", |p, c| {
            Methods::parse_methods(Resources::Manifests)(p, c);
        }));
        parser.add_custom(CustomAttribute::new_with("blobs", |p, c| {
            Methods::parse_methods(Resources::Blobs)(p, c);
        }));
        parser.add_custom(CustomAttribute::new_with("tags", |p, c| {
            Methods::parse_methods(Resources::Tags)(p, c);
        }));
    }
}

impl Project for Proxy {
    fn interpret(_: &lifec::World, _: &lifec::Block) {}

    fn parser() -> lifec::Parser {
        default_parser(Self::world()).with_special_attr::<Proxy>()
    }

    fn runtime() -> lifec::Runtime {
        let mut runtime = default_runtime();
        // TODO -- Change login-acr to something more generic
        runtime.install_with_custom::<LoginACR>("");
        runtime.install_with_custom::<Authenticate>("");
        runtime.install_with_custom::<Mirror>("");
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Resolve>("");
        runtime.install_with_custom::<Discover>("");
        runtime.install_with_custom::<Teleport>("");
        runtime.install_with_custom::<Continue>("");
        runtime.install_with_custom::<Artifact>("");
        runtime.install_with_custom::<LoginOverlayBD>("");
        runtime.install_with_custom::<FormatOverlayBD>("");
        runtime
    }

    fn configure_dispatcher(
        dispatcher_builder: &mut lifec::DispatcherBuilder,
        context: Option<ThunkContext>,
    ) {
        if let Some(context) = context {
            Host::add_start_command_listener::<Self>(context, dispatcher_builder);
        }
    }

    fn on_start_command(&mut self, start_command: Start) {
        let tc = self.context.clone();
        if let Some(handle) = self.context.handle() {
            handle.spawn(async move {
                tc.dispatch_start_command(start_command).await;
            });
        }
    }
}

impl WebApp for Proxy {
    fn create(context: &mut lifec::ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
        let proxy_src = self
            .context
            .state()
            .find_text("proxy_src")
            .expect("should have src for proxy");
        let registry_host = self
            .context
            .find_symbol("registry_host")
            .expect("should have a registry host");
        let registry_name = self
            .context
            .find_symbol("registry_name")
            .expect("should have a registry name");

        if let Some(block) = self.context.block() {
            let context = self.context.clone();
            if let Some(i) = block.index().iter().find(|b| b.root().name() == "proxy") {
                let mut context_map = HashMap::<(Methods, Resources), ThunkContext>::default();

                let mut route = Route::new();
                let graphs = Proxy::extract_routes(i);

                for graph in graphs {
                    let methods = graph
                        .find_symbol_values("method")
                        .iter()
                        .filter_map(|m| Methods::lexer(m).next())
                        .collect::<Vec<_>>();
                    let resources = graph
                        .find_symbol_values("resource")
                        .iter()
                        .filter_map(|r| Resources::lexer(r).next())
                        .collect::<Vec<_>>();

                    for (method, resource) in methods.iter().zip(resources) {
                        let mut context = context.with_state(graph.clone());
                        context
                            .state_mut()
                            .with_bool("proxy_enabled", true)
                            .with_symbol("registry_host", &registry_host)
                            .with_symbol("registry_name", &registry_name)
                            .with_text("proxy_src", proxy_src.to_string());

                        if let Some(proxy_src_path) = graph.find_symbol("proxy_src_path") {
                            event!(
                                Level::TRACE,
                                "Adding proxy_src_path to {:?} {:?}",
                                method,
                                resource
                            );
                            context
                                .state_mut()
                                .with_symbol("proxy_src_path", proxy_src_path);
                        }

                        context_map.insert((method.clone(), resource), context);
                    }
                }

                // Resolve manifest settings
                //
                let get_manifests_settings = context_map
                    .get(&(Methods::Get, Resources::Manifests))
                    .cloned()
                    .unwrap_or_default();

                let head_manifests_settings = context_map
                    .get(&(Methods::Head, Resources::Manifests))
                    .cloned()
                    .unwrap_or_default();

                let put_manifests_settings = context_map
                    .get(&(Methods::Put, Resources::Manifests))
                    .cloned()
                    .unwrap_or_default();

                let delete_manifests_settings = context_map
                    .get(&(Methods::Delete, Resources::Manifests))
                    .cloned()
                    .unwrap_or_default();

                route = route.at(
                    "/:name<[a-zA-Z0-9/_-]+(?:manifests)>/:reference",
                    get(manifests_api
                        .data(get_manifests_settings)
                        .data(self.host.clone()))
                    .head(
                        manifests_api
                            .data(head_manifests_settings)
                            .data(self.host.clone()),
                    )
                    .put(
                        manifests_api
                            .data(put_manifests_settings)
                            .data(self.host.clone()),
                    )
                    .delete(
                        manifests_api
                            .data(delete_manifests_settings)
                            .data(self.host.clone()),
                    ),
                );

                // Resolve blob settings
                //
                let get_blobs_settings = context_map
                    .get(&(Methods::Get, Resources::Blobs))
                    .cloned()
                    .unwrap_or_default();

                let post_blobs_settings = context_map
                    .get(&(Methods::Post, Resources::Blobs))
                    .cloned()
                    .unwrap_or_default();

                let put_blobs_settings = context_map
                    .get(&(Methods::Put, Resources::Blobs))
                    .cloned()
                    .unwrap_or_default();

                let patch_blobs_settings = context_map
                    .get(&(Methods::Patch, Resources::Blobs))
                    .cloned()
                    .unwrap_or_default();

                route = route.at(
                    "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/:digest",
                    get(blob_download_api
                        .data(get_blobs_settings)
                        .data(self.host.clone())),
                );

                route = route.at(
                    "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/uploads",
                    post(
                        blob_upload_api
                            .data(post_blobs_settings)
                            .data(self.host.clone()),
                    ),
                );

                route = route.at(
                    "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/uploads/:reference",
                    put(blob_chunk_upload_api
                        .data(put_blobs_settings)
                        .data(self.host.clone()))
                    .patch(
                        blob_chunk_upload_api
                            .data(patch_blobs_settings)
                            .data(self.host.clone()),
                    ),
                );

                // Resolve tags settings
                //
                let get_tags_settings = context_map
                    .get(&(Methods::Get, Resources::Tags))
                    .cloned()
                    .unwrap_or_default();

                route = route.at(
                    "/:name<[a-zA-Z0-9/_-]+(?:tags)>/list",
                    get(tags_api.data(get_tags_settings).data(self.host.clone())),
                );

                let route = Route::new().nest(
                    "/v2",
                    route.at(
                        "/",
                        get(index.data(self.context.clone()).data(self.host.clone()))
                            .head(index.data(self.context.clone()).data(self.host.clone())),
                    ),
                );

                return route;
            }
        }

        panic!("Could not create routes")
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

impl From<ThunkContext> for Proxy {
    fn from(context: ThunkContext) -> Self {
        let proxy_src = context
            .search()
            .find_text("proxy_src")
            .expect("should have a proxy src");
        Self {
            context,
            host: Arc::new(Host::load_content::<Proxy>(proxy_src)),
        }
    }
}

impl Proxy {
    /// Handles executing the proxy sequence
    ///
    pub async fn handle(host: &Host, input: &ThunkContext) -> Response {
        let (join, _) = if let Some(proxy_src_path) = input.search().find_symbol("proxy_src_path") {
            let replace_host = Host::open::<Proxy>(proxy_src_path)
                .await
                .expect("should open");

            replace_host.execute(&input)
        } else {
            host.execute(&input)
        };

        match join.await {
            Ok(result) => Proxy::into_response(&result),
            Err(err) => {
                event!(Level::ERROR, "Error handling call sequence, {err}");
                Proxy::soft_fail()
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
        use hyper::Client;
        use hyper::StatusCode;
        use hyper_tls::HttpsConnector;
        use lifec::prelude::*;
        use lifec::Source;
        use lifec::ThunkContext;
        use lifec::WorldExt;
        use lifec_poem::WebApp;

        use crate::Proxy;
        let mut host = Host::load_content::<Proxy>(
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
        let src = host.world().fetch::<Source>();
        let mut graph = AttributeGraph::new(index);
        graph.add_text_attr("proxy_src", src.0.to_string());

        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let world = lifec::World::new();
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

            let app = Proxy::create(&mut tc).routes();
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
