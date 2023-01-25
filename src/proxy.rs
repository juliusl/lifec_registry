use lifec::{
    debugger::Debugger,
    engine::{Cleanup, Performance, Profilers},
    guest::{Guest, RemoteProtocol},
    host::EventHandler,
    prelude::{
        AttributeParser, Block, EventRuntime, Host, Journal,
        Parser, Plugins, Run, Sequencer, SpecialAttribute, State, ThunkContext, Value, World, NodeStatus, Node,
    },
    project::{default_parser, default_runtime, default_world, Project, RunmdFile, Workspace},
    runtime::Runtime, appendix::Appendix,
};
use lifec_poem::WebApp;
use poem::Route;
use reality::{store::StoreIndex, Interpreter};
use reality_azure::AzureBlockClient;
use specs::{Entity, LazyUpdate, RunNow, WorldExt};
use std::{collections::HashMap, sync::Arc};
use tokio::io::AsyncReadExt;
use tracing::{event, Level};

use crate::{
    plugins::{
        guest::{AzureAgent, AzureDispatcher, AzureGuest, AzureMonitor},
        LoginNydus
    },
    Artifact, ArtifactManifest, Authenticate, Descriptor, Discover, ImageIndex,
    ImageManifest, Login, LoginACR, LoginOverlayBD, Mirror, RemoteRegistry, Resolve, Teleport,
};

mod proxy_target;
pub use proxy_target::ProxyTarget;

mod manifests;
pub use manifests::Manifests;

mod blobs;
pub use blobs::Blobs;

mod blobs_uploads;
pub use blobs_uploads::BlobsUploads;

mod proxy_route;
pub use proxy_route::ProxyRoute;
use proxy_route::AddRoute;

/// Struct for creating a customizable registry proxy,
///
#[derive(Default)]
pub struct RegistryProxy {
    host: Arc<Host>,
    /// Initial thunk context,
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

    fn parse(parser: &mut AttributeParser, content: impl AsRef<str>) {
        parser.define("app_host", Value::Symbol(content.as_ref().to_string()));

        parser.with_custom::<ProxyRoute<Manifests>>();
        parser.with_custom::<ProxyRoute<Blobs>>();
        parser.with_custom::<ProxyRoute<BlobsUploads>>();
    }
}

impl Project for RegistryProxy {
    fn interpret(world: &World, block: &Block) {
        // Interpret mirror plugins
        Mirror::default().interpret(world, block);
    }

    fn parser() -> Parser {
        let mut world = Self::world();
        let mut handlers = Self::node_handlers();
        {
            let runtime = world.fetch::<Runtime>();

            for (name, handler) in runtime.iter_handlers() {
                handlers.insert(name.to_string(), handler.clone());
            }
        }

        world.insert(handlers);

        default_parser(world).with_special_attr::<RegistryProxy>()
    }

    fn runtime() -> Runtime {
        let mut runtime = default_runtime();
        runtime.install_with_custom::<Run<RegistryProxy>>("");
        runtime.install_with_custom::<LoginACR>("");
        runtime.install_with_custom::<LoginNydus>("");
        runtime.install_with_custom::<LoginOverlayBD>("");
        runtime.install_with_custom::<Teleport>("");
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Authenticate>("");
        runtime.install_with_custom::<Mirror>("");
        runtime.install_with_custom::<Resolve>("");
        runtime.install_with_custom::<Discover>("");
        runtime.install_with_custom::<Artifact>("");
        runtime.install_with_custom::<RemoteRegistry>("");
        runtime.install_with_custom::<AzureGuest>("");
        runtime.install_with_custom::<AzureAgent>("");
        runtime.install_with_custom::<AzureDispatcher>("");
        runtime.install_with_custom::<AzureMonitor>("");
        runtime
    }

    fn world() -> World {
        let mut world = default_world();
        world.insert(Self::runtime());
        world.register::<ProxyRoute<Manifests>>();
        world.register::<ProxyRoute<Blobs>>();
        world.register::<ProxyRoute<BlobsUploads>>();
        world.register::<ImageIndex>();
        world.register::<Descriptor>();
        world.register::<ImageManifest>();
        world.register::<ArtifactManifest>();
        world.register::<NodeStatus>();
        world
    }
}

impl WebApp for RegistryProxy {
    fn create(context: &mut ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
        let route = Route::default()
            .add_route::<Blobs>(&self.host, &self.context)
            .add_route::<Manifests>(&self.host, &self.context)
            .add_route::<BlobsUploads>(&self.host, &self.context);
        
        Route::default()
            .nest("/v2", route)
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

        if context.is_enabled("enable_guest_agent") {
            // Enables guest agent
            // The guest runs seperately from the host's engine
            let guest = build_registry_proxy_guest_agent(&context);
            if context.is_enabled("enable_guest_agent_dispatcher") {
                let protocol = guest.protocol();
                let entity = protocol.as_ref().entities().entity(2);
                protocol.as_ref().system_data::<State>().activate(entity);
            }

            if context.enable_guest(guest) {
                event!(Level::INFO, "Guest agent has been enabled");
            }
        }

        proxy
    }
}

/// Installs guest agent code,
///
fn install_guest_agent(root: &mut Workspace) {
    let account_name = std::env::var("ACCOUNT_NAME").unwrap_or_default();
    root.cache_file(&RunmdFile::new_src(
        "guest",
        format!(
            r#"
        ```
        + .engine
        : .start            setup
        : .start            start,  agent
        : .select           recover
        : .loop
        ```

        ``` setup
        + .runtime
        : .remote_registry
        : .process          sh setup-guest-storage.sh
        : .silent
        ```

        ``` start
        + .runtime
        : .println          Starting guest listener
        : .azure_guest      {account_name}
        : .polling_rate     800 ms
        ```

        ``` agent
        + .runtime
        : .println          Starting guest agent
        : .azure_agent      {account_name}
        : .polling_rate     800 ms
        ```

        ``` recover
        + .runtime
        : .println          Entering recovery mode 
        : .timer 10s
        ```
        "#,
        ),
    ));

    // root.cache_file(&RunmdFile::new_src(
    //     "dispatcher",
    //     format!(
    //         r#"
    //     ```
    //     + .engine
    //     : .start            setup
    //     : .start            start, monitor
    //     : .select           recover
    //     : .loop
    //     ```

    //     ``` setup
    //     + .runtime
    //     : .remote_registry
    //     : .process              sh setup-guest-storage.sh
    //     : .silent
    //     ```

    //     ``` start
    //     + .runtime
    //     : .println              Starting remote dispatcher
    //     : .azure_dispatcher     {account_name}
    //     ```

    //     ``` monitor
    //     + .runtime
    //     : .println              Starting remote monitor
    //     : .azure_monitor        {account_name}
    //     ```

    //     ``` recover
    //     + .runtime
    //     : .println              Entering recovery mode
    //     : .timer 10s
    //     ```
    //     "#,
    //     ),
    // ));
}

/// Builds and returns a registry proxy guest agent,
///
fn build_registry_proxy_guest_agent(tc: &ThunkContext) -> Guest {
    let mut root = tc.workspace().expect("should have a workspace").clone();
    install_guest_agent(&mut root);

    let mut world = root
        .compile::<RegistryProxy>()
        .expect("should compile into a world");
    world.insert(None::<RemoteProtocol>);
    world.insert(None::<Debugger>);
    world.insert(None::<HashMap<Entity, NodeStatus>>);
    world.insert(None::<Vec<Performance>>);
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

    let guest = Guest::new::<RegistryProxy>(entity, host, |guest| {
        let world = guest.protocol();
        let lazy_updates = world.as_ref().read_resource::<LazyUpdate>();

        lazy_updates.exec(|world| {
            EventRuntime::default().run_now(&world);
            Cleanup::default().run_now(&world);
            EventHandler::<Debugger>::default().run_now(&world);

            let profilers = world.system_data::<Profilers>();
            profilers.profile();
        });

        for node in world.as_ref().system_data::<State>().event_nodes() {
            lazy_updates.insert(node.status.entity(), node.status);
        }
    });

    guest.update_protocol(|p| {
        p.as_mut().insert(Some(guest.subscribe()));
        true
    });

    guest
}

/// Builds a guest to interface w/ a remote registry proxy,
///
pub async fn build_registry_proxy_guest_agent_remote(tc: &ThunkContext) -> Guest {
    let mut root = tc.workspace().expect("should have a workspace").clone();
    install_guest_agent(&mut root);

    let mut world = root
        .compile::<RegistryProxy>()
        .expect("should compile into a world");
    world.insert(None::<Debugger>);
    world.insert(None::<HashMap<Entity, NodeStatus>>);
    world.insert(None::<Vec<Performance>>);
    let mut host = Host::from(world);
    host.prepare::<RegistryProxy>();
    host.link_sequences();
    host.build_appendix();
    host.enable_listener::<()>();
    host.prepare::<RegistryProxy>();
    let appendix = host
        .as_mut()
        .remove::<Appendix>()
        .expect("should be able to remove appendix");
    let appendix = Arc::new(appendix);
    host.world_mut().insert(appendix.clone());

    let entity = tc.entity().expect("should have an entity");

    let mut guest = Guest::new::<RegistryProxy>(entity, host, move |guest| {
        let world = guest.protocol();
        let lazy_updates = world.as_ref().read_resource::<LazyUpdate>();

        lazy_updates.exec(|world| {
            EventHandler::<()>::default().run_now(&world);
        });

        lazy_updates.exec(|world| {
            let mut state = world.system_data::<State>();
            if !state.should_exit() && state.can_continue() {
                state.tick();
            }
        });
    });

    guest.update_protocol(|p| {
        p.ensure_encoder::<Journal>();
        p.ensure_encoder::<NodeStatus>();
        p.ensure_encoder::<Performance>();
        true
    });

    guest.add_node(Node {
        appendix,
        status: NodeStatus::Custom(entity),
        display: Some(|state, ui| {
            let mut opened = true;
            ui.window("Proxy store")
                .size([800.0, 600.0], imgui::Condition::Appearing)
                .opened(&mut opened)
                .build(|| {
                    if let Some(rp) = state.remote_protocol.as_ref() {
                        let world = rp.remote.borrow();
                        let index = world.as_ref().try_fetch::<StoreIndex<AzureBlockClient>>();
                        let plugins = world.as_ref().system_data::<Plugins>();
                        if let Some(index) = index.as_ref() {
                            if let Some(t) = ui.begin_table("files", 2) {
                                for e in index.entries() {
                                    let name = format!(
                                        "{}.{}",
                                        e.name().cloned().unwrap_or_default(),
                                        e.symbol().cloned().unwrap_or_default()
                                    );
                                    ui.table_next_row();
                                    ui.table_next_column();
                                    ui.text(&name);

                                    ui.table_next_column();
                                    if ui.button(format!("Download {name}")) {
                                        let blob = plugins
                                            .features()
                                            .handle()
                                            .block_on(async move { e.bytes().await });

                                        plugins.features().handle().spawn(async move {
                                            let mut buf = vec![];
                                            blob.as_ref().read_to_end(&mut buf).await.ok();

                                            match tokio::fs::write(name, &buf).await {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    event!(Level::ERROR, "Error {err}");
                                                }
                                            }
                                        });
                                    }
                                }
                                t.end();
                            }
                        }
                    }
                });

            opened
        }),
        remote_protocol: Some(guest.subscribe()),
        ..Default::default()
    });

    guest.enable_remote();
    guest
}

mod tests {
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_proxy_parsing() {
        use crate::RegistryProxy;
        use hyper::Client;
        use hyper_tls::HttpsConnector;
        use lifec::prelude::*;
        // use lifec_poem::WebApp;
        // use hyper::StatusCode;

        let mut host = Host::load_content::<RegistryProxy>(
            r#"
        # Example proxy definition
        ``` start proxy
        # Proxy setup
        + .proxy localhost:8567
        : .manifests
        : .blobs
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

        let world = World::new();
        let entity = world.entities().create();
        let https = HttpsConnector::new();
        let client = Client::builder().build::<_, hyper::Body>(https);

        host.exit();
        // let runtime = tokio::runtime::Runtime::new().unwrap();
        // let handle = runtime.handle();

        // let mut tc = ThunkContext::default()
        //     .with_block(&block)
        //     .with_state(graph)
        //     .enable_https_client(client)
        //     .enable_async(entity, handle.clone());

        // Temp disable test
        // let app = RegistryProxy::create(&mut tc).routes();
        // let cli = poem::test::TestClient::new(app);

        // let resp = cli
        //     .get("/v2/library/test/manifests/test_ref?ns=test.com")
        //     .send()
        //     .await;
        // resp.assert_status(StatusCode::SERVICE_UNAVAILABLE);

        // let resp = cli
        //     .get("/v2/library/test/blobs/test_digest?ns=test.com")
        //     .send()
        //     .await;
        // resp.assert_status(StatusCode::SERVICE_UNAVAILABLE);

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
    }
}
