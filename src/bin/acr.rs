use clap::{Args, Parser, Subcommand};
use lifec::host::HostSettings;
use lifec::prelude::*;
use lifec_registry::RegistryProxy;
use serde::Serialize;
use std::path::PathBuf;
use tracing::event;
use tracing::Level;
use tracing_subscriber::EnvFilter;

mod teleport;
use teleport::TeleportSettings;

mod mirror;
use mirror::default_mirror_engine;
use mirror::default_mirror_root;

/// Small example tool to convert .runmd to hosts.toml
///
#[tokio::main]
async fn main() {
    let cli = ACR::parse();
    tracing_subscriber::fmt::Subscriber::builder()
    .with_env_filter(if !cli.debug {
        EnvFilter::builder()
            .from_env()
            .expect("should work")
    } else {
        EnvFilter::builder()
            .with_default_directive("lifec_registry=debug".parse().expect("should parse"))
            .from_env()
            .expect("should work")
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
                        event!(
                            Level::ERROR,
                            "mirror_runmd not found, run `acr --registry {registry} init`"
                        );
                        panic!("Uninitialized directory");
                    }
                }
            }

            match command {
                Commands::Open => {
                    let mut host = Host::load_workspace::<RegistryProxy>(
                        None,
                        registry_host,
                        registry,
                        None::<String>,
                        None::<String>,
                    );

                    host.enable_listener::<ACR>();

                    tokio::task::block_in_place(|| {
                        host.open_runtime_editor::<RegistryProxy>();
                    })
                }
                Commands::Mirror(mut host_settings) => {
                    if host_settings.workspace.is_none() {
                        host_settings.set_workspace(format!("{registry}.{registry_host}"));
                    }

                    host_settings.handle::<RegistryProxy>().await;
                }
                Commands::Teleport(teleport) => match teleport {
                    TeleportSettings {
                        repo,
                        command: teleport::Commands::Info,
                        ..
                    } => {
                        let repo_dir =
                            PathBuf::from(format!(".world/{registry_host}/{registry}/{repo}"));

                        teleport.command.info(&repo_dir).await;
                    }
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
                        command,
                    } => {
                        let repo_dir =
                            PathBuf::from(format!(".world/{registry_host}/{registry}/{repo}"));

                        match &command {
                            teleport::Commands::Format
                            | teleport::Commands::Import
                            | teleport::Commands::Convert
                            | teleport::Commands::Link => {
                                command
                                    .execute(format, registry_host, registry, &repo_dir)
                                    .await;
                            }
                            _ => {
                                todo!()
                            }
                        }
                    }
                },
                Commands::Init(_mirror_settings) => {
                    if mirror_runmd.exists() {
                        event!(Level::WARN, "Overwriting existing file {:?}", mirror_runmd);
                    }

                    tokio::fs::write(
                        &mirror_runmd,
                        default_mirror_engine().source.expect("should have a value"),
                    )
                    .await
                    .expect("Should be able to write runmd to file");
                    event!(
                        Level::INFO,
                        "Wrote runmd file, recommend tracking the .world dir with source control"
                    );

                    tokio::fs::write(
                        &world_dir.join(".runmd"),
                        default_mirror_root().source.expect("should have a value"),
                    )
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
                    let mut host = Host::open::<RegistryProxy>(mirror_runmd)
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
    /// Registry host, Ex. azurecr.io, or azurecr-test.io
    #[clap(long, default_value_t=String::from("azurecr.io"))]
    registry_host: String,
    #[clap(subcommand)]
    command: Option<Commands>,
}

/// Enumeration of subcommands
///
#[derive(Subcommand)]
enum Commands {
    /// Opens an editor,
    /// 
    Open,
    /// Host a mirror server that can extend ACR features,
    ///
    Mirror(HostSettings),
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

impl Listener for ACR {
    fn create(_: &World) -> Self {
        ACR::default()
    }

    fn on_runmd(&mut self, _: &RunmdFile) {}

    fn on_status_update(&mut self, _: &StatusUpdate) {}

    fn on_operation(&mut self, _: Operation) { }

    fn on_error_context(&mut self, _: &ErrorContext) {}

    fn on_completed_event(&mut self, _: &Entity) {}

    fn on_start_command(&mut self, _: &Start) {}
}

