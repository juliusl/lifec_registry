use clap::{Args, Parser, Subcommand};
use hyper::StatusCode;
use lifec::{default_runtime, AttributeIndex, Inspector, Interpreter};
use lifec::{Host, Project};
use lifec_registry::{Mirror, MirrorProxy};
use poem::Response;
use serde::Serialize;
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

            if !mirror_runmd.exists() {
                panic!("mirror_runmd not found, run `acr --registry {registry} init`")
            }

            match command {
                Commands::Mirror(mut host) => {
                    host.set_path(
                        mirror_runmd
                            .to_str()
                            .expect("should be able to create string"),
                    );
                    if let Some(mut host) = host.create_host::<ACR>().await.take() {
                        host.handle_start();
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
    /// Note: This will generate a new .runmd file based on the current directory.
    /// This file can be modified to extend the mirror.
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
    fn configure_engine(_: &mut lifec::Engine) {
        // No-op
    }

    fn interpret(world: &lifec::World, block: &lifec::Block) {
        Mirror::<Self>::default().interpret(world, block)
    }

    fn runtime() -> lifec::Runtime {
        let mut runtime = default_runtime();
        runtime.install_with_custom::<Mirror<Self>>("");
        runtime
    }
}
