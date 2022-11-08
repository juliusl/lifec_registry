use lifec::{
    debugger::Debugger,
    engine::{Cleanup, Performance, Profilers},
    guest::{Guest, Monitor, RemoteProtocol, Sender},
    host::EventHandler,
    prelude::{
        Appendix, AttributeParser, Block, Editor, EventRuntime, Host, Journal, NodeStatus, Parser,
        Run, Sequencer, SpecialAttribute, State, ThunkContext, Value, World,
    },
    project::{default_parser, default_runtime, default_world, Project, RunmdFile, Workspace},
    runtime::Runtime,
};
use lifec_poem::{RoutePlugin, WebApp};
use poem::{Route, RouteMethod};
use specs::{Join, RunNow, WorldExt};
use std::{fs::File, path::PathBuf, sync::Arc};
use tracing::{event, Level};

use crate::{
    plugins::{LoginNydus, Store},
    Artifact, ArtifactManifest, Authenticate, Descriptor, Discover, FormatOverlayBD, ImageIndex,
    ImageManifest, Login, LoginACR, LoginOverlayBD, Mirror, RemoteRegistry, Resolve, Teleport,
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
        runtime.install_with_custom::<FormatOverlayBD>("");
        runtime.install_with_custom::<FormatOverlayBD>("");
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Authenticate>("");
        runtime.install_with_custom::<Mirror>("");
        runtime.install_with_custom::<Resolve>("");
        runtime.install_with_custom::<Discover>("");
        runtime.install_with_custom::<Artifact>("");
        runtime.install_with_custom::<Store>("");
        runtime.install_with_custom::<RemoteRegistry>("");
        runtime
    }

    fn world() -> World {
        let mut world = default_world();
        world.insert(Self::runtime());
        world.register::<Manifests>();
        world.register::<Blobs>();
        world.register::<ImageIndex>();
        world.register::<Descriptor>();
        world.register::<ImageManifest>();
        world.register::<ArtifactManifest>();
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

        if context.is_enabled("enable_guest_agent") {
            // Enables guest agent
            // The guest runs seperately from the host's engine
            if context.enable_guest(build_registry_proxy_guest_agent(&context)) {
                event!(Level::INFO, "Guest agent has been enabled");
            }
        }

        proxy
    }
}

/// Installs guest agent code,
///
fn install_guest_agent(root: &mut Workspace) {
    root.cache_file(&RunmdFile::new_src(
        "dispatcher",
        format!(
            r#"
        ```
        + .engine
        : .start        start
        : .start        cooldown
        : .loop
        ```

        ``` start
        + .runtime
        : .watch            .guest-commands
        : .create           file
        : .remote_registry
        : .process          sh send-guest-commands.sh
        ```

        ``` cooldown
        + .runtime
        : .timer 500ms
        ```
        "#,
        ),
    ));

    root.cache_file(&RunmdFile::new_src(
        "guest",
        format!(
            r#"
        ```
        + .engine
        : .start        setup
        : .next      listener
        ```

        ``` setup
        + .runtime
        : .remote_registry
        : .process          sh setup-guest-storage.sh
        : .println          Starting guest listener/monitor
        ```
        "#,
        ),
    ));

    root.cache_file(&RunmdFile::new_src(
        "listener",
        format!(
            r#"
        ```
        + .engine
        : .start start
        : .start cooldown
        : .loop
        ```

        ``` start
        + .runtime
        : .remote_registry
        : .process   sh fetch-guest-commands.sh
        : .listen   .guest-commands
        : .remote_registry
        : .process   sh send-guest-state.sh
        ```

        ``` cooldown
        + .runtime
        : .timer 5 s
        ```
        "#,
        ),
    ));

    let work_dir = root.work_dir().join(".guest");

    match std::fs::create_dir_all(&work_dir) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, "could not create dirs, {err}");
        }
    }

    match std::fs::create_dir_all(&work_dir.join("status")) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, "could not create dirs, {err}");
        }
    }

    match std::fs::create_dir_all(&work_dir.join("performance")) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, "could not create dirs, {err}");
        }
    }

    match std::fs::create_dir_all(&work_dir.join("journal")) {
        Ok(_) => {}
        Err(err) => {
            event!(Level::ERROR, "could not create dirs, {err}");
        }
    }
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
        EventRuntime::default().run_now(guest.protocol().as_ref());
        Cleanup::default().run_now(guest.protocol().as_ref());
        EventHandler::<Debugger>::default().run_now(guest.protocol().as_ref());
        guest
            .protocol()
            .as_ref()
            .system_data::<Profilers>()
            .profile();

        let nodes = guest
            .protocol()
            .as_ref()
            .system_data::<State>()
            .event_nodes();
        for node in nodes {
            guest
                .protocol()
                .as_ref()
                .write_component()
                .insert(node.status.entity(), node.status)
                .expect("should be able to insert status");
        }

        let work_dir = guest.workspace().work_dir().join(".guest");
        guest.update_performance(&work_dir);
        guest.update_status(&work_dir);
        guest.update_journal(&work_dir);
    });

    guest
}

/// Builds a guest to interface w/ a remote registry proxy,
///
pub fn build_registry_proxy_guest_agent_remote(tc: &ThunkContext) -> Guest {
    let mut root = tc.workspace().expect("should have a workspace").clone();
    install_guest_agent(&mut root);

    let mut world = root
        .compile::<RegistryProxy>()
        .expect("should compile into a world");
    world.insert(None::<Debugger>);
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

    let mut guest = Guest::new::<RegistryProxy>(entity, host, |guest| {
        EventHandler::<()>::default().run_now(guest.protocol().as_ref());

        if guest.send_commands(guest.workspace().work_dir().join(".guest-commands")) {
            event!(
                Level::WARN,
                "Commands not sent, previous commands have not been read"
            );
        }

        let workspace = guest.workspace().clone();
        if guest.update_protocol(move |protocol| {
            fn read_stream<'a>(name: &'a PathBuf) -> impl FnOnce() -> File + 'a {
                move || {
                    std::fs::OpenOptions::new()
                        .read(true)
                        .open(name)
                        .ok()
                        .unwrap()
                }
            }

            let work_dir = workspace.work_dir().join(".guest");
            let performance_dir = work_dir.join("performance");
            let control = performance_dir.join("control");
            let frames = performance_dir.join("frames");
            let blob = performance_dir.join("blob");

            let performance_updated = if control.exists() && frames.exists() && blob.exists() {
                protocol.clear::<Performance>();
                protocol.receive::<Performance, _, _>(
                    read_stream(&control),
                    read_stream(&frames),
                    read_stream(&blob),
                );

                std::fs::remove_file(control).ok();
                std::fs::remove_file(frames).ok();
                std::fs::remove_file(blob).ok();
                true
            } else {
                false
            };

            let status_dir = work_dir.join("status");
            let control = status_dir.join("control");
            let frames = status_dir.join("frames");
            let blob = status_dir.join("blob");
            let status_updated = if control.exists() && frames.exists() && blob.exists() {
                protocol.clear::<NodeStatus>();
                protocol.receive::<NodeStatus, _, _>(
                    read_stream(&control),
                    read_stream(&frames),
                    read_stream(&blob),
                );

                std::fs::remove_file(control).ok();
                std::fs::remove_file(frames).ok();
                std::fs::remove_file(blob).ok();
                true
            } else {
                false
            };

            let remote_dir = work_dir.join("journal");
            let control = remote_dir.join("control");
            let frames = remote_dir.join("frames");
            let blob = remote_dir.join("blob");
            let journal_updated = if control.exists() && frames.exists() && blob.exists() {
                protocol.clear::<Journal>();
                protocol.receive::<Journal, _, _>(
                    read_stream(&control),
                    read_stream(&frames),
                    read_stream(&blob),
                );
                std::fs::remove_file(control).ok();
                std::fs::remove_file(frames).ok();
                std::fs::remove_file(blob).ok();
                true
            } else {
                false
            };

            performance_updated | status_updated | journal_updated
        }) {
            event!(
                Level::TRACE,
                "Updated remote guest state, {:?}",
                guest.workspace().work_dir()
            );
        }
    });
    guest.enable_remote();
    guest
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
