use clap::{Args, Parser};
use serde::Serialize;
use std::path::PathBuf;

use super::Commands;
/// ACR CLI
///
#[derive(Default, Parser, Clone)]
#[clap(name = "acr")]
#[clap(arg_required_else_help = true)]
#[clap(about = "Provides extensions and modifications for container runtimes that work with ACR")]
pub struct ACR {
    /// Name of the registry to use, also referred to as a "Tenant",
    ///
    /// If None, then the context is set to the default host workspace,
    ///
    #[clap(long)]
    pub registry: Option<String>,
    /// Enable debug mode
    #[clap(long, short, action)]
    pub debug: bool,
    /// If guest is passed, the mirror will enable a guest agent in addition to the mirror,
    ///
    /// The guest agent communicates over azure storage, and the name passed here will be used
    /// as the azure storage account name to communicate with.
    ///
    #[clap(long)]
    pub guest: Option<String>,
    /// Registry host, Ex. azurecr.io, or azurecr-test.io
    #[clap(long, default_value_t=String::from("azurecr.io"))]
    pub registry_host: String,
    #[clap(subcommand)]
    pub command: Option<Commands>,
}

impl ACR {
    /// Handle cli state,
    /// 
    pub async fn handle(&self) {
        match self {
            ACR {
                registry,
                registry_host,
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
                command.clone().handle(self.clone(), self.debug, mirror_runmd, world_dir).await;
            }
            _ => {
                unreachable!()
            }
        }
    }
}

/// Settings to use when initializing a .runmd template for the mirror engine
///
#[derive(Args, Serialize, Clone)]
pub struct MirrorSettings {
    /// Operating system this mirror will operate on,
    ///
    /// Currently, only ubuntu is implemented.
    ///
    #[clap(long, default_value_t = String::from("ubuntu"))]
    pub operating_system: String,
    /// Streaming image format to use,
    ///
    /// Currently, only overlaybd is implemented.
    ///
    #[clap(long, default_value_t = String::from("overlaybd"))]
    pub teleport_format: String,
    /// Login script,
    ///
    /// Currently, only signing in from az cli is implemented.
    ///
    #[clap(long, default_value_t = String::from("lib/sh/login-acr.sh"))]
    pub login_script: String,
    /// Address that the mirror will be hosted on
    ///
    /// Currently, only http is supported by default, but https can be enabled by editing runmd,
    /// TODO - Write up how to do this,
    ///
    #[clap(long, default_value_t = String::from("localhost:8578"))]
    pub mirror_address: String,
    /// Host domain of the upstream registry,
    ///
    /// The upstream registry is used to discover teleportable images,
    ///
    #[clap(long, default_value_t = String::from("azurecr.io"))]
    pub registry_host: String,
    /// If initializing settings, only initialize the hosts.toml file
    ///
    #[clap(long, action)]
    pub init_hosts_config_only: bool,
    /// Root of the current filesystem,
    ///
    /// This is usually just `/` however when testing it's useful to specify since root is a privelaged folder.
    ///
    #[clap(long)]
    pub fs_root: Option<String>,
    /// Name of the registry,
    ///
    #[clap(skip)]
    pub registry_name: Option<String>,
    /// Artifact type to use,
    ///
    #[clap(skip)]
    pub artifact_type: Option<String>,
}
