use clap::{Args, Parser, Subcommand};
use lifec::host::HostSettings;
use lifec::prelude::*;
use lifec_registry::RegistryProxy;
use lifec_registry::{
    Artifact, Authenticate, Continue, Discover, FormatOverlayBD, Import, Login, LoginACR,
    LoginOverlayBD, Mirror, Resolve, Teleport,
};
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
                    let host = Host::load_workspace::<ACR>(None, registry_host, registry, None::<String>, None::<String>);

                    tokio::task::block_in_place(|| {
                        host.open_runtime_editor::<ACR>();
                    })
                }
                Commands::Mirror(host) => {
                    if let Some(mut host) = host.create_host::<ACR>().await.take() {
                        host.start::<ACR>();
                    } else {
                        panic!("Could not create/start host");
                    }
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

                    tokio::fs::write(&world_dir.join(".runmd"), 
                    r#"
                    ```
                    ```
                    "#)
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
                            for route in RegistryProxy::extract_routes(i) {
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
                                        // if let Some(prop) = properties.property(event.1 .0) {
                                        //     print!(" {prop}");
                                        // }
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

impl Project for ACR {
    fn interpret(world: &World, block: &Block) {
        Mirror::default().interpret(world, block);

        for index in block
            .index()
            .iter()
            .filter(|b| b.root().name() == "runtime")
        {
            for (child, props) in index.iter_children() {
                if props.property("mirror").is_some() {
                    let child = world.entities().entity(*child);
                    if let Some(graph) = world.write_component::<AttributeGraph>().get_mut(child) {
                        graph.add_text_attr("proxy_src", "");
                    }
                }
            }
        }
    }

    fn parser() -> lifec::prelude::Parser {
        default_parser(Self::world()).with_special_attr::<RegistryProxy>()
    }

    fn runtime() -> Runtime {
        let mut runtime = default_runtime();
        runtime.install_with_custom::<Run<Self>>("");
        runtime.install_with_custom::<Authenticate>("");
        runtime.install_with_custom::<LoginACR>("");
        runtime.install_with_custom::<Mirror>("");
        runtime.install_with_custom::<Login>("");
        runtime.install_with_custom::<Resolve>("");
        runtime.install_with_custom::<Import>("");
        runtime.install_with_custom::<Discover>("");
        runtime.install_with_custom::<Teleport>("");
        runtime.install_with_custom::<Artifact>("");
        runtime.install_with_custom::<Continue>("");
        runtime.install_with_custom::<LoginOverlayBD>("");
        runtime.install_with_custom::<FormatOverlayBD>("");
        runtime
    }
}

