use clap::{Args, Subcommand};

mod init;
pub use init::Init;

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
    /// Format images from a repo,
    /// 
    Format,
    Link,
}
