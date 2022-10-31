use lifec::{
    prelude::{
        AttributeParser, Block, Host, Parser, Run, SpecialAttribute, ThunkContext, Value, World,
    },
    project::{default_parser, default_runtime, default_world, Project},
    runtime::Runtime,
};
use lifec_poem::{RoutePlugin, WebApp};
use poem::{Route, RouteMethod};
use specs::{Join, WorldExt};
use std::sync::Arc;

use crate::{
    Artifact, Authenticate, Discover, FormatOverlayBD, Login, LoginACR,
    LoginOverlayBD, Mirror, Resolve, Teleport, plugins::LoginNydus,
};

mod proxy_target;
pub use proxy_target::ProxyTarget;

mod manifests;
pub use manifests::Manifests;

mod blobs;
pub use blobs::Blobs;

/// Struct for creating a customizable registry proxy,
///
#[derive(Default)]
pub struct RegistryProxy {
    host: Arc<Host>,
    context: ThunkContext,
}

impl RegistryProxy {
    /// Returns the host,
    /// 
    pub fn host(&self) -> Arc<Host> {
        self.host.clone()
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
        parser.with_custom::<Blobs>();
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
        runtime.install_with_custom::<LoginNydus>("");
        runtime.install_with_custom::<LoginOverlayBD>("");
        runtime.install_with_custom::<Teleport>("");
        runtime.install_with_custom::<FormatOverlayBD>("");
        runtime.install_with_custom::<FormatOverlayBD>("");
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Authenticate>("");
        runtime.install_with_custom::<Mirror>("");
        runtime.install_with_custom::<Resolve>("");
        runtime.install_with_custom::<Discover>("");
        runtime.install_with_custom::<Artifact>("");
        runtime
    }

    fn world() -> World {
        let mut world = default_world();
        world.insert(Self::runtime());
        world.register::<Manifests>();
        world.register::<Blobs>();
        world
    }
}

impl WebApp for RegistryProxy {
    fn create(context: &mut ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
        let mut route = Route::default();

        let mut manifest_route = None::<RouteMethod>;
        for manifest in self.host.world().read_component::<Manifests>().join() {
        
            if manifest.can_route() {
                let mut manifest = manifest.clone();
                manifest.set_host(self.host.clone());
                manifest.set_context(self.context.clone());

                if let Some(m) = manifest_route.take() {
                    manifest_route = Some(manifest.route(Some(m)));
                } else {
                    manifest_route = Some(manifest.route(None));
                }
            }
        }
        let path = "/:repo<[a-zA-Z0-9/_-]+(?:manifests)>/:reference";
        if let Some(manifest_route) = manifest_route.take() {
            route = route.at(path, manifest_route);
        }

        let mut blob_route = None::<RouteMethod>;
        for blob in self.host.world().read_component::<Blobs>().join() {
        
            if blob.can_route() {
                let mut blob = blob.clone();
                blob.set_host(self.host.clone());
                blob.set_context(self.context.clone());

                if let Some(m) = blob_route.take() {
                    blob_route = Some(blob.route(Some(m)));
                } else {
                    blob_route = Some(blob.route(None));
                }
            }
        }
        let path = "/:repo<[a-zA-Z0-9/_-]+(?:blobs)>/:reference";
        if let Some(blob_route) = blob_route.take() {
            route = route.at(path, blob_route);
        }

        Route::default().nest("/v2", route)
    }
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

        let proxy = Self {
            host: Arc::new(host),
            context: context.clone(),
        };

        // if context.enable_guest(proxy.host.clone()) {
        //     event!(Level::DEBUG, "Guest enabled for proxy");
        // }

        proxy
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
