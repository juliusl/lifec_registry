use std::collections::HashMap;

use hyper::http::StatusCode;
use lifec::{
    default_parser, default_runtime, AttributeGraph, AttributeIndex, BlockIndex, CustomAttribute,
    Executor, Host, Project, Runtime, SpecialAttribute, Start, ThunkContext, Value,
};
use lifec_poem::WebApp;
use tracing::event;
use tracing::Level;
mod methods;
use logos::Logos;
use methods::Methods;

mod resources;
use poem::{
    delete, get, handler, head, patch, post, put,
    web::{Data, Path, Query},
    EndpointExt, Request, Response, Route,
};
use resources::Resources;
use serde::Deserialize;

use crate::{Authenticate, Discover, Login, Resolve};

/// Struct for creating a customizable registry proxy,
///
#[derive(Default)]
pub struct Proxy {
    context: ThunkContext,
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
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Authenticate>("");
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

impl WebApp for Proxy {
    fn create(context: &mut lifec::ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
        if let Some(block) = self.context.block() {
            let context = self.context.clone();
            if let Some(i) = block.index().iter().find(|b| b.root().name() == "proxy") {
                let (route, _) = Proxy::extract_routes(i, Route::new(), move |mut r, graph| {
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

                    let context_map = HashMap::<(Methods, Resources), ThunkContext>::default();
                    // Todo bring this outside, just change this to return graphs
                    // That way you could technically customize each individual method/resource pair
                    // for (method, resource) in methods.iter().zip(resources)
                    // {
                    //     let context = context.clone();
                    //     match (method, resource) {
                    //         (Methods::Get, Resources::Manifests) => {}
                    //         (Methods::Head, Resources::Manifests) => {}
                    //         (Methods::Put, Resources::Manifests) => {}
                    //         (Methods::Delete, Resources::Manifests) => {}
                    //         (Methods::Get, Resources::Blobs) => {}
                    //         (Methods::Get, Resources::Tags) => {}
                    //         (Methods::Put, Resources::Blobs) => {}
                    //         (Methods::Patch, Resources::Blobs) => {}
                    //         (Methods::Post, Resources::Blobs) => {}
                    //         _ => continue,
                    //     }
                    // }

                    if resources.iter().all(|r| *r == Resources::Manifests) {
                        let get_manifests_data = context_map
                            .get(&(Methods::Get, Resources::Manifests))
                            .cloned()
                            .unwrap_or_default();

                        let head_manifests_data = context_map
                            .get(&(Methods::Head, Resources::Manifests))
                            .cloned()
                            .unwrap_or_default();

                        let put_manifests_data = context_map
                            .get(&(Methods::Put, Resources::Manifests))
                            .cloned()
                            .unwrap_or_default();

                        let delete_manifests_data = context_map
                            .get(&(Methods::Delete, Resources::Manifests))
                            .cloned()
                            .unwrap_or_default();

                        r = r.at(
                            "/:name<[a-zA-Z0-9/_-]+(?:manifests)>/:reference",
                            get(manifests_api.data(get_manifests_data))
                                .head(manifests_api.data(head_manifests_data))
                                .put(manifests_api.data(put_manifests_data))
                                .delete(manifests_api.data(delete_manifests_data)),
                        );
                    }

                    if resources.iter().all(|r| *r == Resources::Blobs) {

                    }

                    if resources.iter().all(|r| *r == Resources::Tags) {

                    }

                    r
                });

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
    dispatcher: Data<&ThunkContext>,
) -> Response {
    event!(Level::DEBUG, "Got /v2 request");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();

    if let Some(ns) = ns {
        input.state_mut().with_symbol("ns", &ns);
    }

    todo!()
}

#[derive(Deserialize)]
struct ManifestAPIParams {
    ns: String,
}
/// Resolves an image
/// 
#[handler]
async fn manifests_api(
    request: &Request,
    method: poem::http::Method,
    Path((name, reference)): Path<(String, String)>,
    Query(ManifestAPIParams { ns }): Query<ManifestAPIParams>,
    dispatcher: Data<&ThunkContext>,
) -> Response {
    let name = name.trim_end_matches("/manifests");
    event!(
        Level::DEBUG,
        "Got resolve request, repo: {name} ref: {reference} host: {ns}"
    );
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input
        .state_mut()
        .with_symbol("repo", name)
        .with_symbol("reference", reference)
        .with_symbol("ns", &ns)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("accept", request.header("accept").unwrap_or_default())
        .with_symbol("method", method);

    let mut host = Host::load_content::<Proxy>(input.state().find_text("proxy_src").unwrap());

    let input = host.execute(&input);
    Proxy::into_response(&input)
}

#[derive(Deserialize)]
struct TagsAPIParams {
    ns: String,
}
#[handler]
async fn tags_api(
    request: &Request,
    Path(name): Path<String>,
    Query(TagsAPIParams { ns }): Query<TagsAPIParams>,
    dispatcher: Data<&ThunkContext>,
) -> Response {
    let name = name.trim_end_matches("/tags");

    event!(Level::DEBUG, "Got list_tags request, {name}");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input
        .state_mut()
        .with_symbol("ns", ns)
        .with_symbol("name", name);

    todo!()
}

#[handler]
async fn blob_download_api(
    request: &Request,
    Path((name, digest)): Path<(String, String)>,
    Query(ManifestAPIParams { ns }): Query<ManifestAPIParams>,
    dispatcher: Data<&ThunkContext>,
) -> Response {
    let name = name.trim_end_matches("/blobs");
    event!(Level::DEBUG, "Got download_blobs request, {name} {digest}");
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("ns", &ns)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("digest", digest);

    if let Some(accept) = request.header("accept") {
        input.state_mut().add_text_attr("accept", accept)
    }

    todo!()
}

#[derive(Deserialize)]
struct UploadParameters {
    digest: Option<String>,
    ns: String,
}
#[handler]
async fn blob_chunk_upload_api(
    request: &Request,
    method: poem::http::Method,
    Path((name, reference)): Path<(String, String)>,
    Query(UploadParameters { digest, ns }): Query<UploadParameters>,
    dispatcher: Data<&ThunkContext>,
) -> Response {
    let name = name.trim_end_matches("/blobs");

    event!(
        Level::DEBUG,
        "Got {method} blob_upload_chunks request, {name} {reference}, {:?}",
        digest
    );
    event!(Level::TRACE, "{:#?}", request);

    let mut input = dispatcher.clone();
    input
        .state_mut()
        .with_symbol("name", name)
        .with_symbol("reference", reference)
        .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()))
        .with_symbol("digest", digest.unwrap_or_default());

    todo!()
}

#[derive(Deserialize)]
struct ImportParameters {
    digest: Option<String>,
    mount: Option<String>,
    from: Option<String>,
    ns: String,
}
#[handler]
async fn blob_upload_api(
    request: &Request,
    Path(name): Path<String>,
    Query(ImportParameters {
        digest,
        mount,
        from,
        ns,
    }): Query<ImportParameters>,
    dispatcher: Data<&ThunkContext>,
) -> Response {
    let name = name.trim_end_matches("/blobs");

    if let (Some(mount), Some(from)) = (mount, from) {
        event!(
            Level::DEBUG,
            "Got blob_import request, {name}, {mount}, {from}"
        );
        event!(Level::TRACE, "{:#?}", request);

        let mut input = dispatcher.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("mount", mount)
            .with_symbol("from", from)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        todo!()
    } else if let Some(digest) = digest {
        event!(
            Level::DEBUG,
            "Got blob_upload_monolith request, {name}, {digest}"
        );
        event!(Level::TRACE, "{:#?}", request);

        let mut input = dispatcher.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("digest", digest)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        todo!()
    } else if let None = digest {
        event!(Level::DEBUG, "Got blob_upload_session_id request, {name}");
        event!(Level::TRACE, "{:#?}", request);

        let mut input = dispatcher.clone();
        input
            .state_mut()
            .with_symbol("name", name)
            .with_symbol("api", format!("https://{ns}/v2{}", request.uri().path()));

        todo!()
    } else {
        todo!() // soft_fail
    }
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
    pub fn extract_routes(
        block_index: &BlockIndex,
        mut route: Route,
        on_route: impl Fn(Route, &AttributeGraph) -> Route,
    ) -> (Route, Vec<AttributeGraph>) {
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
                        route = on_route(route, &graph);
                        graphs.push(graph);
                    }
                    _ => continue,
                }
            }

            let graph = graph.unscope();
            route = on_route(route, &graph);
            graphs.push(graph);
        }

        (route, graphs)
    }
}

mod tests {
    use lifec::AttributeGraph;

    #[test]
    #[tracing_test::traced_test]
    fn test_proxy_parsing() {
        use lifec::prelude::*;
        use lifec::Executor;
        use lifec::ThunkContext;
        use poem::Route;

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
            let (_, graphs) = Proxy::extract_routes(&index, Route::new(), |r, graph| {
                for (name, value) in graph.values() {
                    eprintln!("{name}\n\t{:#?}", value);
                }
                eprintln!();
                r
            });

            let graph = graphs.first().expect("should be a graph");
            let tc = ThunkContext::default().with_state(graph.clone());

            let _ = host.execute(&tc);
        }
    }
}
