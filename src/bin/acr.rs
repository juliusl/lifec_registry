use clap::{Args, Parser, Subcommand};
use lifec::{InspectExtensions, default_runtime, Interpreter, ReadStorage, Sequence, Join};
use lifec::{Host, Project};
use lifec_registry::{MirrorProxy, Mirror};
use serde::Serialize;
use tracing_subscriber::EnvFilter;
use std::path::PathBuf;
use tinytemplate::TinyTemplate;
use tracing::event;
use tracing::Level;

/// Template user's runmd mirror file,
///
static MIRROR_TEMPLATE: &'static str = r#"
# ACR Mirror 
- This file is generated per registry host
- It provides a mirror server that facilitates the teleport feature on the host machine
- This file can be edited to customize settings

## Control Settings 
- Engine sequence when the mirror starts

``` mirror
+ .engine
: .event install
: .event start
: .loop
```
## Install mirror components
- The overlaybd snapshotter is the current teleport provider,
- This section can be expanded, once new providers are available.

``` install mirror
+ .runtime
: .process lifec 
: .flag --runmd_path lib/overlaybd/setup_env
: .arg start
: .flag --engine_name {operating_system}
```

## Start the mirror server
- When this event is called it will start a server that will operate indefinitely,
- If an error occurs, it should restart the server after going through the setup process once more 

``` start mirror
: src_dir         .symbol lib
: work_dir        .symbol .work/acr
: file_src        .symbol .work/acr/access_token
: teleport_format .symbol {teleport_format}

+ .runtime
: .process  sh {login_script}
:  REGISTRY_NAME .env {registry_name}

: .install  access_token

: .mirror   {registry_host}
: .host     {mirror_address}, resolve
```
"#;

/// Small example tool to convert .runmd to hosts.toml
///
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    let cli = ACR::parse();
    match cli {
        ACR {
            registry,
            registry_host,
            command: Some(command),
        } => {
            let world_dir = PathBuf::from(".world").join(&registry_host).join(&registry);
            let mirror_runmd = world_dir.join("mirror.runmd");

            tokio::fs::create_dir_all(&world_dir)
                .await
                .expect("Should be able to make directories");

            match command {
                Commands::Mirror(_) => {
                    todo!("mirror")
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

                    let rendered = tt
                        .render("mirror", &mirror_settings)
                        .expect("Should be able to render template");

                    tokio::fs::write(mirror_runmd, rendered)
                        .await
                        .expect("Should be able to write runmd to file");
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

#[derive(Args)]
struct TeleportSettings {
    /// Repository name,
    ///
    /// Note: ORAS artifacts w/ the referrers api doesn't currently support cross repo,
    /// This setting must be the same for both src/dst repositories
    ///
    #[clap(long)]
    repo: String,
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
    #[clap(long, default_value_t = String::from("lib/sh/azure-login.sh"))]
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
}

impl MirrorProxy for ACR {
    fn resolve_response(_: &lifec::ThunkContext) -> poem::Response {
        todo!()
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
