use hyper::http::StatusCode;
use lifec::{
    default_parser, default_runtime, AttributeGraph, AttributeIndex, BlockIndex, CustomAttribute,
    Host, Project, Runtime, SpecialAttribute, Start, ThunkContext, Value,
};
use lifec_poem::WebApp;
use logos::Logos;
use poem::{
    get, handler, post, put,
    web::{Data, Query},
    EndpointExt, Request, Response, Route,
};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{event, Level};

use crate::{Authenticate, Discover, Login, LoginACR, Mirror, Resolve};

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
        runtime.install_with_custom::<Authenticate>("");
        runtime.install_with_custom::<LoginACR>("");
        runtime.install_with_custom::<Mirror>("");
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Resolve>("");
        runtime.install_with_custom::<Discover>("");
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

/*
Table of OCI Endpoints

ID	Method	API Endpoint	Success	Failure
end-1	GET	/v2/	                                                                            200	404/401



end-8a	GET	        /v2/<name>/tags/list	                                                    200	404
end-8b	GET	        /v2/<name>/tags/list                  ?n=<integer>&last=<integer>	        200	404

end-3	GET / HEAD	/v2/<name>/manifests/<reference>	                                        200	404
end-7	PUT	        /v2/<name>/manifests/<reference>	                                        201	404
end-9	DELETE	    /v2/<name>/manifests/<reference>	                                        202	404/400/405
*/

impl WebApp for Proxy {
    fn create(context: &mut lifec::ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
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
                        let mut context = context.clone();
                        context.state_mut().with_bool("proxy_enabled", true);
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
                    get(manifests_api.data(get_manifests_settings))
                        .head(manifests_api.data(head_manifests_settings))
                        .put(manifests_api.data(put_manifests_settings))
                        .delete(manifests_api.data(delete_manifests_settings)),
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
                    get(blob_download_api.data(get_blobs_settings)),
                );

                route = route.at(
                    "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/uploads",
                    post(blob_upload_api.data(post_blobs_settings)),
                );

                route = route.at(
                    "/:name<[a-zA-Z0-9/_-]+(?:blobs)>/uploads/:reference",
                    put(blob_chunk_upload_api.data(put_blobs_settings))
                        .patch(blob_chunk_upload_api.data(patch_blobs_settings)),
                );

                // Resolve tags settings
                //
                let get_tags_settings = context_map
                    .get(&(Methods::Get, Resources::Tags))
                    .cloned()
                    .unwrap_or_default();

                route = route.at(
                    "/:name<[a-zA-Z0-9/_-]+(?:tags)>/list",
                    get(tags_api.data(get_tags_settings)),
                );

                let route = Route::new().nest(
                    "/v2",
                    route.at(
                        "/",
                        get(index.data(self.context.clone()))
                            .head(index.data(self.context.clone())),
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
        Self { context }
    }
}

impl Proxy {
    pub fn into_response(context: &ThunkContext) -> Response {
        if let Some(body) = context.state().find_binary("body") {
            let content_type = context
                .state()
                .find_text("content-type")
                .expect("A content type should've been provided");
            let digest = context
                .state()
                .find_text("digest")
                .expect("A digest should've been provided");

            Response::builder()
                .status(StatusCode::OK)
                .content_type(content_type)
                .header("Docker-Content-Digest", digest)
                .body(body)
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
        use lifec::prelude::*;

        use crate::Proxy;
        let mut host = Host::load_content::<Proxy>(
            r#"
        # Example proxy definition
        ``` start proxy
        # Proxy setup
        + .proxy                  localhost:8567

        ## Resolve manifests and artifacts
        : .manifests head, get
        : .println test
        : .println that
        : .println sequence
        : .println works
        # :   .login                  access_token
        # :   .authn                  oauth2
        # :   .resolve                application/vnd.oci.image.manifest.v1+json <if accept is * or matches>
        # :   .discover               dadi.image.v1
        # :   .discover               sbom.json

        ## Teleport and dispatch a convert operation if teleport isn't available
        # :   .teleport               overlaybd, auto
        # :   .converter              convert overlaybd <name of the engine that can do the conversion>

        ## Validate signatures
        # :   .notary

        ## Download blobs
        # : .blobs head, get
        # : .login                  access_token
        # : .authn                  oauth2
        # : .println
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

        for index in block.index() {
            let graphs = Proxy::extract_routes(&index);

            let graphs = graphs.first().expect("should be a graph");
            // let tc = ThunkContext::default().with_state(graph.clone());

            // let _ = host.execute(&tc);
        }
    }
}

// #[test]
// #[tracing_test::traced_test]
// fn test_mirror() {
//     use hyper::Client;
//     use hyper_tls::HttpsConnector;
//     use lifec::WorldExt;

//     tokio::runtime::Runtime::new().unwrap().block_on(async {
//         let world = lifec::World::new();
//         let entity = world.entities().create();
//         let https = HttpsConnector::new();
//         let client = Client::builder().build::<_, hyper::Body>(https);
//         let runtime = tokio::runtime::Runtime::new().unwrap();
//         let handle = runtime.handle();
//         let mut tc = ThunkContext::default()
//             .enable_https_client(client)
//             .enable_async(entity, handle.clone());

//         let app = Mirror::create(&mut tc).routes();
//         let cli = poem::test::TestClient::new(app);

//         let resp = cli.get("/").send().await;
//         resp.assert_status(StatusCode::NOT_FOUND);

//         let resp = cli.head("/").send().await;
//         resp.assert_status(StatusCode::NOT_FOUND);

//         let resp = cli.get("/v2").send().await;
//         resp.assert_status_is_ok();

//         let resp = cli.get("/v2/").send().await;
//         resp.assert_status_is_ok();

//         let resp = cli.head("/v2").send().await;
//         resp.assert_status_is_ok();

//         let resp = cli.head("/v2/").send().await;
//         resp.assert_status_is_ok();

//         let resp = cli
//             .get("/v2/library/test/manifests/test_ref?ns=test.com")
//             .send()
//             .await;
//         resp.assert_status_is_ok();

//         let resp = cli
//             .head("/v2/library/test/manifests/test_ref?ns=test.com")
//             .send()
//             .await;
//         resp.assert_status_is_ok();

//         let resp = cli
//             .put("/v2/library/test/manifests/test_ref?ns=test.com")
//             .send()
//             .await;
//         resp.assert_status_is_ok();

//         let resp = cli
//             .delete("/v2/library/test/manifests/test_ref?ns=test.com")
//             .send()
//             .await;
//         resp.assert_status_is_ok();

//         // let resp = cli
//         //     .get("/v2/library/test/blobs/test_digest?ns=test.com")
//         //     .send()
//         //     .await;
//         // resp.assert_status_is_ok();

//         // let resp = cli
//         //     .post("/v2/library/test/blobs/uploads?ns=test.com")
//         //     .send()
//         //     .await;
//         // resp.assert_status_is_ok();

//         // let resp = cli
//         //     .patch("/v2/library/test/blobs/uploads/test?ns=test.com")
//         //     .send()
//         //     .await;
//         // resp.assert_status_is_ok();

//         // let resp = cli
//         //     .put("/v2/library/test/blobs/uploads/test?ns=test.com")
//         //     .send()
//         //     .await;
//         // resp.assert_status_is_ok();

//         // let resp = cli
//         //     .get("/v2/library/test/tags/list?ns=test.com")
//         //     .send()
//         //     .await;
//         // resp.assert_status_is_ok();

//         runtime.shutdown_background();
//     });
// }
