
/// Struct for cli config for making images teleportable
/// 
#[derive(Args)]
pub struct Teleport {
    /// Streaming format the image should use,
    /// 
    #[clap(long, default_value_t=String::from("overlaybd"))]
    format: String,
    /// The repo that should be teleportable,
    /// 
    #[clap(long)]
    repo: String,
}