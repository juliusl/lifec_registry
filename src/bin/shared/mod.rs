#[doc(hidden)]
#[macro_use]
pub mod macros;

mod cli;
use cli::ACR;
use cli::MirrorSettings;

mod commands;
use commands::Commands;

mod mirror;
use mirror::default_mirror_engine;
use mirror::default_mirror_root;
use tracing_subscriber::EnvFilter;

/// Starts the cli,
/// 
pub async fn start() {
    use clap::Parser;

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
    
    cli.handle().await;
}