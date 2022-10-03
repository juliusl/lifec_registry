use clap::{Args, Parser, Subcommand};
use hyper::StatusCode;
use lifec::{
    default_runtime, AttributeGraph, AttributeIndex, Block, Engine, Inspector, Interpreter, Source,
    Start, ThunkContext, WorldExt,
};
use lifec::{Host, Project};
use lifec_registry::{LoginACR, Mirror, MirrorProxy};
use poem::Response;
use serde::Serialize;
use std::collections::HashMap;
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
                        let block = {
                            let block_entity = Engine::find_block(host.world(), "start mirror")
                                .expect("runmd requires a `start mirror` block");
                            let block = &host.world().read_component::<Block>();
                            let block = block
                                .get(block_entity)
                                .expect("should have a block")
                                .clone();
                            block
                        };

                        if let Some(start) = host.find_start("start mirror") {
                            if let Some(proxy_block) =
                                block.index().iter().find(|b| b.root().name() == "proxy")
                            {
                                let graph = AttributeGraph::new(proxy_block.clone());
                                let mut context =
                                    ThunkContext::default().with_state(graph).with_block(&block);

                                // Set up state
                                {
                                    let src = host.world().read_resource::<Source>();
                                    context.state_mut()
                                        .with_text("proxy_src", &src.0)
                                        .with_symbol("registry", registry)
                                        .with_symbol("registry_host", registry_host);
                                }
                                host.world_mut()
                                    .write_component()
                                    .insert(start, context)
                                    .expect("should be able to insert thunk context");
                            }
                        }

                        if let Some(lifec::Commands::Start(start)) = host.command(){
                            match start {
                                Start {
                                    id: None, 
                                    engine_name: None,
                                    ..
                                } => {
                                    host.set_command(lifec::Commands::start_engine("start mirror"));
                                },
                                _ => {

                                }
                            }
                        }

                        host.handle_start::<ACR>();
                    } else {
                        panic!("Could not create/start host");
                    }
                }
                Commands::Teleport(_) => {
                    todo!("teleport")
                }
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

impl MirrorProxy for ACR {
    fn resolve_response(tc: &lifec::ThunkContext) -> poem::Response {
        if let Some(body) = tc.state().find_binary("body") {
            let content_type = tc
                .state()
                .find_text("content-type")
                .expect("A content type should've been provided");
            let digest = tc
                .state()
                .find_text("digest")
                .expect("A digest should've been provided");

            Response::builder()
                .status(StatusCode::OK)
                .content_type(content_type)
                .header("Docker-Content-Digest", digest)
                .body(body)
        } else {
            // Fall-back response
            Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .finish()
        }
    }

    fn resolve_error(_: String, _: &lifec::ThunkContext) -> poem::Response {
        todo!()
    }
}

impl Project for ACR {
    fn interpret(world: &lifec::World, block: &lifec::Block) {
        Mirror::<Self>::default().interpret(world, block)
    }

    fn runtime() -> lifec::Runtime {
        let mut runtime = default_runtime();
        runtime.install_with_custom::<Mirror<Self>>("");
        runtime.install_with_custom::<LoginACR>("");
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
            if let Self { command: Some(Commands::Mirror(mut host)), .. } = ACR::from(tc.clone()) {
                host.start::<ACR>(id, Some(tc));
            }
        }
    }
}

impl From<ThunkContext> for ACR {
    fn from(tc: ThunkContext) -> Self {
        if let Some(proxy_src) = tc.state().find_text("proxy_src") {
            Self {
                registry: tc.state().find_symbol("registry").expect("should be in state"),
                registry_host: tc.state().find_symbol("registry_host").expect("should be in state"),
                debug: false,
                command: Some(Commands::Mirror(Host::load_content::<ACR>(proxy_src))),
            }
        } else {
            panic!("proxy_src was not included")
        }
    }
}
