use std::{ops::Deref, path::PathBuf};

use clap::{Args, Subcommand};

mod init;
pub use init::Init;
use lifec::prelude::{
    AttributeGraph, Block, Engine, Executor, Host, Inspector, SecureClient, ThunkContext, WorldExt,
};
use tracing::{event, Level};

use crate::{MirrorSettings, ACR};

/// Struct for cli config for making images teleportable
///
#[derive(Default, Args)]
pub struct TeleportSettings {
    /// Streaming format the image should use,
    ///
    #[clap(long, default_value_t=String::from("overlaybd"))]
    pub format: String,
    /// The repo that should be teleportable,
    ///
    #[clap(long)]
    pub repo: String,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Default, Subcommand)]
pub enum Commands {
    /// Prints information within the current context on the status of teleport,
    ///
    #[default]
    Info,
    /// Initialize a formatting environment for a repo
    ///
    Init(Init),
    /// Format images from a repo, basically runs import -> convert -> link
    ///
    Format,
    /// Import a public source image to the registry,
    ///
    Import,
    /// Converts an image from the registry to a streamable format,
    ///
    Convert,
    /// Link an image and it's streamable format,
    ///
    Link,
}

impl Commands {
    /// Dumps information on each tag in the context, 
    /// 
    pub async fn info(
        &self,
        repo_dir: &PathBuf,
    ) {
        let mut read_dir = tokio::fs::read_dir(repo_dir)
            .await
            .expect("should be able to read dir");

        while let Ok(Some(dir_entry)) = read_dir.next_entry().await {
            if dir_entry.file_type().await.unwrap().is_dir() {
                let format_runmd = dir_entry.path().join(".runmd");
                let mut host = Host::open::<ACR>(format_runmd)
                    .await
                    .expect("should be a host");

                host.print_engine_event_graph();
                host.print_lifecycle_graph();
            }
        }
    }

    /// Executes format, link,
    ///
    pub async fn execute(
        &self,
        format: impl AsRef<str>,
        registry_host: impl AsRef<str>,
        registry: impl AsRef<str>,
        repo_dir: &PathBuf,
    ) {
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
                    registry_host: registry_host.as_ref().to_string(),
                    registry_name: Some(registry.as_ref().to_string()),
                    teleport_format: format.as_ref().to_string(),
                    login_script: String::default(),
                    artifact_type: None,
                    operating_system: String::default(),
                    mirror_address: String::default(),
                });

                let block_name = match self {
                    // In this case the whole engine needs to run
                    Commands::Format => "",
                    Commands::Import => "import",
                    Commands::Convert => "convert",
                    Commands::Link => "link",
                    _ => {
                        panic!("This command cannot be executed with this fn")
                    }
                };

                let start = Engine::find_block(
                    host.world(),
                    format!("{} {}", block_name, format.as_ref()).trim(),
                )
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
                    let mut context = context.enable_async(start, runtime.handle().clone());
                    context.enable_https_client(client.deref().clone());

                    let (join, _) = host.execute(&context.with_state(graph.clone()));
                    match join.await {
                        Ok(_) => {}
                        Err(err) => {
                            event!(Level::ERROR, "Error handling call sequence, {err}");
                        }
                    }
                }

                host.exit();
            }
        }
    }
}
