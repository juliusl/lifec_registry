use clap::{Args, Parser, Subcommand};
use lifec::host::HostSettings;
use lifec::prelude::*;
use lifec_registry::hosts_config::DefaultHost;
use lifec_registry::hosts_config::MirrorHost;
use lifec_registry::RegistryProxy;
use serde::Serialize;
use std::path::PathBuf;
use tracing::event;
use tracing::Level;
use tracing_subscriber::EnvFilter;

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
            EnvFilter::builder().from_env().expect("should work")
        } else {
            EnvFilter::builder()
                .from_env()
                .expect("should work")
                .add_directive("reality_azure=debug".parse().expect("should parse"))
                .add_directive("lifec_registry=debug".parse().expect("should parse"))
                .add_directive("lifec=debug".parse().expect("should parse"))
        })
        .compact()
        .init();

    match cli {
        ACR {
            registry,
            registry_host,
            guest,
            command: Some(command),
            ..
        } => {
            let mut world_dir = PathBuf::from(".world").join(&registry_host);
            if let Some(registry) = registry.as_ref() {
                world_dir = world_dir.join(&registry);
            }

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
                            "mirror_runmd not found, run `acr --registry {{registry}} init`"
                        );
                        panic!("Uninitialized directory");
                    }
                }
            }

            match command {
                Commands::Open => {
                    let host = if let Some(registry) = registry.as_ref() {
                        Host::load_workspace::<RegistryProxy>(
                            None,
                            registry_host,
                            registry,
                            None::<String>,
                            None::<String>,
                        )
                    } else {
                        Host::load_default_workspace::<RegistryProxy>(
                            None,
                            registry_host,
                            None::<String>,
                        )
                    };

                    if let Some(guest) = guest.as_ref() {
                        std::env::set_var("ACCOUNT_NAME", guest);
                    }

                    tokio::task::block_in_place(|| {
                        host.open_runtime_editor::<RegistryProxy>(cli.debug);
                    })
                }
                Commands::Mirror(mut host_settings) => {
                    if host_settings.workspace.is_none() {
                        let registry = registry
                            .as_ref()
                            .map_or(String::default(), |v| v.to_string());

                        host_settings.set_workspace(format!("{registry}.{registry_host}"));
                    }

                    if let Some(guest) = guest.as_ref() {
                        std::env::set_var("ACCOUNT_NAME", guest);
                    }

                    if let Some(mut host) = host_settings.create_host::<RegistryProxy>().await {
                        host.enable_listener::<()>();
                        host.start_with::<RegistryProxy>("mirror");
                    } else {
                        host_settings.handle::<RegistryProxy>().await;
                    }
                }
                Commands::Init(MirrorSettings {
                    mirror_address,
                    teleport_format,
                    registry_host,
                    fs_root,
                    hosts_config_only,
                    ..
                }) => {
                    if mirror_runmd.exists() {
                        event!(Level::WARN, "Overwriting existing file {:?}", mirror_runmd);
                    }

                    let hosts_config = if let Some(registry) = registry.as_ref() {
                        MirrorHost::get_hosts_config(
                            format!("{registry}.{registry_host}"),
                            mirror_address,
                            true,
                            Some(teleport_format),
                        )
                    } else {
                        DefaultHost::get_hosts_config(
                            mirror_address,
                            true,
                            Some(registry_host),
                            Some(teleport_format),
                        )
                    };
                    
                    match hosts_config.install(fs_root)
                    {
                        Ok(path) => event!(
                            Level::INFO,
                            "Wrote hosts.toml for host, {:?}",
                            path
                        ),
                        Err(err) => panic!("Could not write hosts.toml {err}"),
                    }

                    if hosts_config_only {
                        event!(Level::INFO, "Skipping .runmd initialization");
                        return;
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
    /// Name of the registry to use, also referred to as a "Tenant",
    ///
    /// If None, then the context is set to the default host workspace,
    ///
    #[clap(long)]
    registry: Option<String>,
    /// Enable debug logging
    #[clap(long, short, action)]
    debug: bool,
    /// If guest is passed, the mirror will enable a guest agent in addition to the mirror,
    ///
    /// The guest agent communicates over azure storage, and the name passed here will be used
    /// as the azure storage account name to communicate with.
    ///
    #[clap(long)]
    guest: Option<String>,
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
    /// If initializing settings, only initialize the hosts.toml file
    /// 
    #[clap(long, action)]
    hosts_config_only: bool,
    /// Root of the current filesystem,
    ///
    /// This is usually just `/` however when testing it's useful to specify since root is a privelaged folder.
    ///
    #[clap(long)]
    fs_root: Option<String>,
    /// Name of the registry,
    ///
    #[clap(skip)]
    registry_name: Option<String>,
    /// Artifact type to use,
    ///
    #[clap(skip)]
    artifact_type: Option<String>,
}
