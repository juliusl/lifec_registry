use lifec::{
    appendix::Appendix,
    debugger::Debugger,
    engine::{Cleanup, Performance, Profilers},
    guest::{Guest, RemoteProtocol},
    host::EventHandler,
    prelude::{
        AttributeParser, Block, EventRuntime, Host, Journal, Node, NodeStatus, Parser, Plugins,
        Run, Sequencer, SpecialAttribute, State, ThunkContext, Value, World,
    },
    project::{default_parser, default_runtime, default_world, Project, RunmdFile, Workspace},
    runtime::Runtime,
};
use lifec_poem::WebApp;
use poem::{Route, handler, get, EndpointExt, web::Data};
use reality::store::StoreIndex;
use reality_azure::AzureBlockClient;
use specs::{Entity, LazyUpdate, RunNow, WorldExt};
use std::{collections::HashMap, sync::Arc, path::{Path, PathBuf}, ffi::OsStr};
use tokio::io::AsyncReadExt;
use tracing::{event, Level};

use crate::{
    plugins::{
        guest::{AzureAgent, AzureDispatcher, AzureGuest, AzureMonitor},
        LoginNydus,
    },
    Artifact, ArtifactManifest, Authenticate, Descriptor, Discover, ImageIndex, ImageManifest,
    Login, LoginACR, LoginOverlayBD, Mirror, RemoteRegistry, Resolve, Teleport, default_access_provider,
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
use proxy_route::AddRoute;
pub use proxy_route::ProxyRoute;

mod auth;
use auth::handle_auth;

/// Struct for creating a customizable registry proxy,
///
/// This proxy is a server that intercepts registry requests intended for upstream registries,
///
#[derive(Default)]
pub struct RegistryProxy {
    /// Thunk context is used to communicate with the underlying runtime,
    context: ThunkContext,
}

impl SpecialAttribute for RegistryProxy {
    fn ident() -> &'static str {
        "proxy"
    }

    fn parse(parser: &mut AttributeParser, content: impl AsRef<str>) {
        // This sets the local host address
        parser.define("app_host", Value::Symbol(content.as_ref().to_string()));

        // This allows for specific methods under registry resources to be configured
        parser.with_custom::<ProxyRoute<Manifests>>();
        parser.with_custom::<ProxyRoute<Blobs>>();
        parser.with_custom::<ProxyRoute<BlobsUploads>>();
    }
}

impl Project for RegistryProxy {
    fn interpret(_: &World, _: &Block) {
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
        // The default runtime gives us all of the built-in plugins from the framework,
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
        // The default_world registers built-in Component types from the framework, as well as some build-in Resources
        let mut world = default_world();

        // The runtime is a resource that can be used to generate executable events
        world.insert(Self::runtime());

        // Component types specific to the registry
        world.register::<ProxyRoute<Manifests>>();
        world.register::<ProxyRoute<Blobs>>();
        world.register::<ProxyRoute<BlobsUploads>>();
        world.register::<ImageIndex>();
        world.register::<Descriptor>();
        world.register::<ImageManifest>();
        world.register::<ArtifactManifest>();

        // This is to enable custom tooling ui
        world.register::<NodeStatus>();
        world
    }
}

/// The proxy is a server so it implements the WebApp trait that will setup routes/handlers
///
impl WebApp for RegistryProxy {
    fn create(context: &mut ThunkContext) -> Self {
        Self::from(context.clone())
    }

    fn routes(&mut self) -> poem::Route {
        let workspace = self.context.workspace().expect("should have a work_dir");

        if let Some(world) = workspace.compile::<RegistryProxy>() {
            let host = Host::from(world);
            let host = Arc::new(host);

            let route = Route::default()
                .add_route::<Blobs>(&host, &self.context)
                .add_route::<Manifests>(&host, &self.context)
                .add_route::<BlobsUploads>(&host, &self.context);

            let file_provider = workspace.work_dir().join("access_token");
            let file_provider = if file_provider.exists() {
                Some(file_provider)
            } else {
                None
            };

            Route::default()
                .at("/status", get(status_check).data(self.context.clone()))
                .at("/auth", get(handle_auth).data(self.context.clone()).data(default_access_provider(file_provider)))
                .nest("/v2", route)
        } else {
            panic!("Cannot start w/o config")
        }
    }
}

impl From<ThunkContext> for RegistryProxy {
    fn from(context: ThunkContext) -> Self {
        let proxy = Self {
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

/// Runs a status check
/// 
#[handler]
async fn status_check(
    context: Data<&ThunkContext>
) -> String {
    format!("{:#?}", context.workspace())
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
        use hyper::StatusCode;
        use lifec::prelude::*;
        use lifec_poem::WebApp;
        use std::ops::Deref;
        use std::time::Duration;
        use tokio::time::Instant;
        use tracing::debug;

        let root = r#"
        # Example proxy definition
        ```
        + .operation test_manifest
        : .println not_teleporting manifest

        + .operation test_blobs
        : .println not_teleporting blobs

        + overlaybd .operation test_manifest
        : .println teleporting manifest

        + overlaybd .operation test_blobs
        : .println teleporting blobs

        # Proxy setup
        + .proxy localhost:8567
        : .manifests
        : .get      test_manifest
        : .blobs
        : .get      test_blobs
        ```
        "#;

        let mut workspace = Workspace::new("test.com", Some(std::path::PathBuf::from(".test")));
        workspace.set_root_runmd(root);

        let world = RegistryProxy::compile_workspace(&workspace, vec![].iter(), None);
        let mut host = Host::from(world);
        host.link_sequences();
        let mut dispatcher = host.prepare::<RegistryProxy>();
        let mut context = host.world().system_data::<State>().new_context();

        {
            // Test that operation map is what we expect
            let operation_map = host
                .world()
                .fetch::<std::collections::HashMap<String, Entity>>();
            debug!("operation_map: {:?}", operation_map.deref());
            assert!(operation_map.get("adhoc-test_manifest#overlaybd").is_some());
            assert!(operation_map.get("adhoc-test_blobs#overlaybd").is_some());
            assert!(operation_map.get("adhoc-test_manifest").is_some());
            assert!(operation_map.get("adhoc-test_blobs").is_some());
        }

        let app = RegistryProxy::create(&mut context).routes();
        let cli = poem::test::TestClient::new(app);
        let cli = std::sync::Arc::new(cli);

        // Test a request that doesn't have the upgrade to streaming header
        let test_1 = cli.clone();
        tokio::spawn(async move {
            let resp = test_1
                .get("/v2/library/test/manifests/testref?ns=test.com")
                .send()
                .await;
            resp.assert_status(StatusCode::SERVICE_UNAVAILABLE);
        });

        // Test a request that does have the upgrade to streaming header
        let test_2 = cli.clone();
        tokio::spawn(async move {
            let resp = test_2
                .get("/v2/library/test/manifests/testref?ns=test.com")
                .header("x-ms-upgrade-if-streamable", "overlaybd")
                .send()
                .await;
            resp.assert_status(StatusCode::SERVICE_UNAVAILABLE);
        });

        // Test a request w accept if suffix header rejects irrelevant host
        let test_3 = cli.clone();
        tokio::spawn(async move {
            let resp = test_3
                .get("/v2/library/test/manifests/testref?ns=tenant.test.com")
                .header("x-ms-upgrade-if-streamable", "overlaybd")
                .header("x-ms-accept-if-suffix", "registry.io")
                .send()
                .await;
            resp.assert_status(StatusCode::SERVICE_UNAVAILABLE);
        });

        // Test a request w accept if suffix header accepts relevant host, and enables teleport file
        let test_4 = cli.clone();
        tokio::spawn(async move {
            let resp = test_4
                .get("/v2/library/test/manifests/testref?ns=tenant.registry.io")
                .header("x-ms-upgrade-if-streamable", "overlaybd")
                .header("x-ms-accept-if-suffix", "registry.io")
                .header("x-ms-enable-mirror-if-suffix", "registry.io")
                .send()
                .await;
            resp.assert_status(StatusCode::SERVICE_UNAVAILABLE);
        });

        // It's important that all requests start before this line, otherwise the host will exit immediately b/c there will be no operations pending
        host.async_wait_for_exit(
            Some(Instant::now() + Duration::from_millis(100)),
            Duration::from_secs(1000),
            &mut dispatcher,
        )
        .await;

        // TODO: Weird test bug, likely won't affect actual runtime but should investigate
        let entity = host.world().entities().entity(7);
        host.start_event(entity);
        host.wait_for_exit(&mut dispatcher);

        assert!(logs_contain("tag: None"));
        assert!(logs_contain(r#"tag: Some("overlaybd")"#));
        assert!(logs_contain(r#"Rejecting host "tenant.test.com""#));
        assert!(!logs_contain(r#"Rejecting host "tenant.registry.io""#));
        host.exit();
    }
}
