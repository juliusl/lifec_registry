use clap::{Args, Parser, Subcommand};
use lifec::{
    default_parser, default_runtime, AttributeGraph, AttributeIndex, Block, BlockProperties,
    Engine, Event, Executor, Inspector, Interpreter, SecureClient, Sequence, Source, Start,
    ThunkContext, Value, WorldExt,
};
use lifec::{Host, Project};
use lifec_registry::{
    Artifact, Authenticate, Continue, Discover, Download, FormatOverlayBD, Login, LoginACR,
    LoginOverlayBD, Mirror, Proxy, Resolve, Teleport,
};
use serde::Serialize;
use std::ops::Deref;
use std::path::PathBuf;
use tinytemplate::TinyTemplate;
use tracing::event;
use tracing::Level;
use tracing_subscriber::EnvFilter;

mod teleport;
use teleport::{TeleportSettings, MIRROR_TEMPLATE};

/// Small example tool to convert .runmd to hosts.toml
///
#[tokio::main]
async fn main() {
    let cli = ACR::parse();
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(if !cli.debug {
            EnvFilter::builder()
                .with_default_directive("acr=info".parse().expect("should parse"))
                .from_env()
                .expect("should work")
                .add_directive("lifec=info".parse().expect("should be ok"))
        } else {
            EnvFilter::builder()
                .with_default_directive("acr=debug".parse().expect("should parse"))
                .from_env()
                .expect("should work")
                .add_directive("lifec=debug".parse().expect("should be ok"))
        })
        .compact()
        .init();

    match cli {
        ACR {
            registry,
            registry_host,
            command: Some(command),
            ..
        } => {
            let world_dir = PathBuf::from(".world").join(&registry_host).join(&registry);
            let mirror_runmd = world_dir.join("mirror.runmd");

            tokio::fs::create_dir_all(&world_dir)
                .await
                .expect("Should be able to make directories");

            // Is there a mirror.runmd file?
            match command {
                Commands::Init(_) => {}
                _ => {
                    if !mirror_runmd.exists() {
                        panic!("mirror_runmd not found, run `acr --registry {registry} init`")
                    }
                }
            }

            match command {
                Commands::Mirror(mut host) => {
                    host.set_path(
                        mirror_runmd
                            .to_str()
                            .expect("should be able to create string"),
                    );
                    if let Some(mut host) = host.create_host::<ACR>().await.take() {
                        if let Some(lifec::Commands::Start(start)) = host.command() {
                            match start {
                                Start {
                                    id: None,
                                    engine_name: None,
                                    ..
                                } => {
                                    host.set_command(lifec::Commands::start_engine("mirror"));
                                }
                                _ => {}
                            }
                        }

                        host.handle_start::<ACR>();
                    } else {
                        panic!("Could not create/start host");
                    }
                }
                Commands::Teleport(teleport) => match teleport {
                    TeleportSettings {
                        format,
                        repo,
                        command: teleport::Commands::Info,
                    } => {}
                    TeleportSettings {
                        format,
                        repo,
                        command: teleport::Commands::Init(mut init),
                    } => {
                        init.registry_host = registry_host;
                        init.registry_name = registry;
                        init.repo = repo;
                        init.format = format;
                        init.init().await;
                    }
                    TeleportSettings {
                        format,
                        repo,
                        command: teleport::Commands::Format,
                    } => {
                        let repo_dir =
                            PathBuf::from(format!(".world/{registry_host}/{registry}/{repo}"));
                    }
                    TeleportSettings {
                        format,
                        repo,
                        command: teleport::Commands::Link,
                    } => {
                        let repo_dir =
                            PathBuf::from(format!(".world/{registry_host}/{registry}/{repo}"));

                        let mut read_dir = tokio::fs::read_dir(repo_dir)
                            .await
                            .expect("should be able to read dir");

                        while let Ok(Some(dir_entry)) = read_dir.next_entry().await {
                            if dir_entry.file_type().await.unwrap().is_dir() {
                                let format_runmd = dir_entry.path().join(".runmd");
                                let mut host = Host::open::<ACR>(format_runmd)
                                    .await
                                    .expect("should be a host");
                                host.world_mut().insert(MirrorSettings {
                                    registry_host: registry_host.to_string(),
                                    registry_name: Some(registry.to_string()),
                                    teleport_format: format.to_string(),
                                    login_script: String::default(),
                                    artifact_type: None,
                                    operating_system: String::default(),
                                    mirror_address: String::default(),
                                });
                                let start =
                                    Engine::find_block(host.world(), format!("link {format}"))
                                        .expect("should be the start");

                                let mut disp = Host::dispatcher_builder().build();
                                disp.setup(host.world_mut());

                                {
                                    let blocks = host.world().read_component::<Block>();
                                    let runtime = host.world().fetch::<tokio::runtime::Runtime>();
                                    let client = host.world().fetch::<SecureClient>();
                                    let block = blocks.get(start).expect("should have a block");

                                    let index = block
                                        .index()
                                        .iter()
                                        .find(|i| i.root().name() == "runtime")
                                        .expect("should have an index")
                                        .clone();
                                    let graph = AttributeGraph::new(index.clone());

                                    let context = ThunkContext::default();
                                    let mut context =
                                        context.enable_async(start, runtime.handle().clone());
                                    context.enable_https_client(client.deref().clone());

                                    let (join, _) =
                                        host.execute(&context.with_state(graph.clone()));
                                    match join.await {
                                        Ok(_) => {
                                            
                                        }
                                        Err(err) => {
                                            event!(
                                                Level::ERROR,
                                                "Error handling call sequence, {err}"
                                            );
                                        }
                                    }
                                }

                                host.exit();
                            }
                        }
                    }
                    _ => {}
                },
                Commands::Init(mut mirror_settings) => {
                    if mirror_runmd.exists() {
                        event!(Level::WARN, "Overwriting existing file {:?}", mirror_runmd);
                    }

                    let mut tt = TinyTemplate::new();
                    tt.add_template("mirror", MIRROR_TEMPLATE)
                        .expect("Should be able to add template");

                    mirror_settings.registry_name = Some(registry.to_string());

                    if mirror_settings.teleport_format == "overlaybd" {
                        mirror_settings.artifact_type = Some("dadi.image.v1".to_string());
                    }

                    let rendered = tt
                        .render("mirror", &mirror_settings)
                        .expect("Should be able to render template");

                    tokio::fs::write(&mirror_runmd, rendered)
                        .await
                        .expect("Should be able to write runmd to file");
                    event!(
                        Level::INFO,
                        "Wrote runmd file, recommend tracking the .world dir with source control"
                    );
                    println!(
                        "{}",
                        mirror_runmd
                            .canonicalize()
                            .expect("should exist")
                            .to_str()
                            .expect("should be able to get str")
                    );
                }
                Commands::Dump => {
                    let mut host = Host::open::<ACR>(mirror_runmd)
                        .await
                        .expect("Should be able to open runmd file");

                    host.print_engine_event_graph();
                    host.print_lifecycle_graph();

                    // Print the proxy state
                    for block in host.world().read_component::<Block>().as_slice() {
                        for i in block.index().iter().filter(|b| b.root().name() == "proxy") {
                            println!("Proxy routes:");
                            println!("This is the configuration for the proxy sub-engine hosted by the mirror");
                            println!();
                            for route in Proxy::extract_routes(i) {
                                let methods = route.find_symbol_values("method");
                                let resources = route.find_symbol_values("resource");

                                let mut zipped = methods.iter().zip(resources.iter());
                                if let Some((method, resource)) = zipped.next() {
                                    print!("\t{:2}:\t{resource} - {method}", route.entity_id());
                                }
                                for (method, _) in zipped {
                                    print!(" {method}");
                                }
                                println!();

                                for i in
                                    route
                                        .find_values("sequence")
                                        .iter()
                                        .filter_map(|v| match v {
                                            Value::Int(ent) => {
                                                Some(host.world().entities().entity(*ent as u32))
                                            }
                                            _ => None,
                                        })
                                {
                                    let props = host.world().read_component::<BlockProperties>();
                                    let events = host.world().read_component::<Event>();
                                    if let (Some(event), Some(properties)) =
                                        (events.get(i), props.get(i))
                                    {
                                        print!("\t{:2}:\t{}", i.id(), event);
                                        if let Some(prop) = properties.property(event.1 .0) {
                                            print!(" {prop}");
                                        }
                                        println!();
                                    }
                                }
                                println!();
                            }
                            return;
                        }
                    }
                }
            }
        }
        _ => {
            unreachable!()
        }
    }
}

/// ACR CLI
///
#[derive(Default, Parser)]
#[clap(name = "acr")]
#[clap(arg_required_else_help = true)]
#[clap(about = "Provides extensions and modifications for container runtimes that work with ACR")]
struct ACR {
    /// Name of the registry to use
    #[clap(long)]
    registry: String,
    /// Enable debug logging
    #[clap(long, short, action)]
    debug: bool,
    /// Registry host, for example azurecr.io, or azurecr-test.io
    #[clap(long, default_value_t=String::from("azurecr.io"))]
    registry_host: String,
    #[clap(subcommand)]
    command: Option<Commands>,
}

/// Enumeration of subcommands
///
#[derive(Subcommand)]
enum Commands {
    /// Host a mirror server that can extend ACR features,
    ///
    Mirror(Host),
    /// Enable image streaming for an image in acr,
    ///
    /// ## Current Streaming Formats
    /// * Overlaybd - (TODO add more info)
    ///
    Teleport(TeleportSettings),
    /// Initialize mirror settings for a particular acr registry,
    ///
    Init(MirrorSettings),
    /// Prints diagnostic information about mirror components,
    ///
    Dump,
}

/// Settings to use when initializing a .runmd template for the mirror engine
///
#[derive(Args, Serialize)]
struct MirrorSettings {
    /// Operating system this mirror will operate on,
    ///
    /// Currently, only ubuntu is implemented.
    ///
    #[clap(long, default_value_t = String::from("ubuntu"))]
    operating_system: String,
    /// Streaming image format to use,
    ///
    /// Currently, only overlaybd is implemented.
    ///
    #[clap(long, default_value_t = String::from("overlaybd"))]
    teleport_format: String,
    /// Login script,
    ///
    /// Currently, only signing in from az cli is implemented.
    ///
    #[clap(long, default_value_t = String::from("lib/sh/login-acr.sh"))]
    login_script: String,
    /// Address that the mirror will be hosted on
    ///
    /// Currently, only http is supported by default, but https can be enabled by editing runmd,
    /// TODO - Write up how to do this,
    ///
    #[clap(long, default_value_t = String::from("localhost:8578"))]
    mirror_address: String,
    /// Host domain of the upstream registry,
    ///
    /// The upstream registry is used to discover teleportable images,
    ///
    #[clap(long, default_value_t = String::from("azurecr.io"))]
    registry_host: String,
    /// Name of the registry,
    ///
    #[clap(skip)]
    registry_name: Option<String>,
    /// Artifact type to use,
    ///
    #[clap(skip)]
    artifact_type: Option<String>,
}

impl Project for ACR {
    fn interpret(world: &lifec::World, block: &lifec::Block) {
        Mirror::default().interpret(world, block);

        let source = world.fetch::<Source>();
        for index in block
            .index()
            .iter()
            .filter(|b| b.root().name() == "runtime")
        {
            for (child, props) in index.iter_children() {
                if props.property("mirror").is_some() {
                    let child = world.entities().entity(*child);
                    if let Some(graph) = world.write_component::<AttributeGraph>().get_mut(child) {
                        graph.add_text_attr("proxy_src", source.0.to_string());
                    }
                }
            }
        }

        for index in block.index().iter().filter(|b| b.root().name() == "proxy") {
            for (child, _) in index.iter_children() {
                let child = world.entities().entity(*child);
                if let Some(graph) = world.write_component::<AttributeGraph>().get_mut(child) {
                    // graph
                    //     .with_symbol("registry_host", &mirror_settings.registry_host)
                    //     .with_symbol(
                    //         "registry_name",
                    //         mirror_settings
                    //             .registry_name
                    //             .as_ref()
                    //             .expect("should have a registry name"),
                    //     );
                }
            }
        }
    }

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
        runtime.install_with_custom::<Download>("");
        runtime.install_with_custom::<Discover>("");
        runtime.install_with_custom::<Teleport>("");
        runtime.install_with_custom::<Artifact>("");
        runtime.install_with_custom::<Continue>("");
        runtime.install_with_custom::<LoginOverlayBD>("");
        runtime.install_with_custom::<FormatOverlayBD>("");
        runtime
    }

    fn configure_dispatcher(
        _dispatcher_builder: &mut lifec::DispatcherBuilder,
        _context: Option<lifec::ThunkContext>,
    ) {
        if let Some(_context) = _context {
            Host::add_start_command_listener::<ACR>(_context, _dispatcher_builder);
        }
    }

    fn on_start_command(&mut self, start_command: lifec::Start) {
        if let Start {
            id: Some(id),
            thunk_context: Some(tc),
            ..
        } = start_command
        {
            // This will create a new host and start the command
            if let Self {
                command: Some(Commands::Mirror(mut host)),
                ..
            } = ACR::from(tc.clone())
            {
                host.start::<ACR>(id, Some(tc));
            }
        }
    }
}

impl From<ThunkContext> for ACR {
    fn from(tc: ThunkContext) -> Self {
        if let Some(proxy_src) = tc.state().find_text("proxy_src") {
            Self {
                registry: tc
                    .state()
                    .find_symbol("registry")
                    .expect("should be in state"),
                registry_host: tc
                    .state()
                    .find_symbol("registry_host")
                    .expect("should be in state"),
                debug: false,
                command: Some(Commands::Mirror(Host::load_content::<ACR>(proxy_src))),
            }
        } else {
            panic!("proxy_src was not included")
        }
    }
}
