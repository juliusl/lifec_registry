use clap::Subcommand;
use lifec::host::HostSettings;
use lifec::prelude::*;
use lifec_registry::hosts_config::DefaultHost;
use lifec_registry::hosts_config::MirrorHost;
use lifec_registry::ContainerdConfig;
use lifec_registry::RegistryProxy;
use std::path::PathBuf;
use tracing::error;
use tracing::event;
use tracing::info;
use tracing::Level;

use super::default_mirror_engine;
use super::default_mirror_root;
use super::MirrorSettings;
use super::ACR;

/// Enumeration of subcommands
///
#[derive(Subcommand, Clone)]
pub enum Commands {
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

impl Commands {
    cfg_editor! {
        pub async fn handle(self, acr: ACR, debug: bool, mirror_runmd: PathBuf, world_dir: PathBuf) {
            match self {
                Commands::Open => {
                    let host = if let Some(registry) = acr.registry.as_ref() {
                        Host::load_workspace::<RegistryProxy>(
                            None,
                            acr.registry_host,
                            registry,
                            None::<String>,
                            None::<String>,
                        )
                    } else {
                        Host::load_default_workspace::<RegistryProxy>(
                            None,
                            acr.registry_host,
                            None::<String>,
                        )
                    };

                    if let Some(guest) = acr.guest.as_ref() {
                        std::env::set_var("ACCOUNT_NAME", guest);
                    }

                    tokio::task::block_in_place(|| {
                        host.open_runtime_editor::<RegistryProxy>(debug);
                    })
                }
                _ => {
                    self.default_handle(acr, debug, mirror_runmd, world_dir).await;
                }
            }
        }
    }

    cfg_not_editor! {
        pub async fn handle(self, acr: ACR, debug: bool, mirror_runmd: PathBuf, world_dir: PathBuf) {
            self.default_handle(acr, debug, mirror_runmd, world_dir).await;
        }
    }

    async fn default_handle(
        self,
        ACR {
            registry,
            registry_host,
            guest,
            ..
        }: ACR,
        _: bool,
        mirror_runmd: PathBuf,
        world_dir: PathBuf,
    ) {
        match self {
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
                min_init,
                ..
            }) => {
                if mirror_runmd.exists() {
                    event!(Level::WARN, "Overwriting existing file {:?}", mirror_runmd);
                }
                
                if min_init {
                    enable_containerd_config().await;

                    let host_config = if let Some(registry) = registry.as_ref() {
                        MirrorHost::get_hosts_config(
                            format!("{registry}.{registry_host}"),
                            mirror_address.to_string(),
                            true,
                            Some(teleport_format.to_string()),
                        )
                    } else {
                        DefaultHost::get_hosts_config(
                            format!("http://{}", mirror_address),
                            true,
                            Some(registry_host.to_string()),
                            Some(teleport_format.to_string()),
                        )
                    };
    
                    match host_config.install(fs_root.to_owned()) {
                        Ok(path) => event!(Level::INFO, "Wrote hosts.toml for host, {:?}", path),
                        Err(err) => panic!("Could not write hosts.toml {err}"),
                    }
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
            _ => {}
        }
    }
}

/// Enable containerd config,
///
async fn enable_containerd_config() {
    // Configure containerd
    let ctr_config = match ContainerdConfig::try_load(None).await {
        Ok(config) => config,
        Err(_) => ContainerdConfig::new(),
    };

    let mut updated = ctr_config
        .enable_overlaybd_snapshotter()
        .enable_hosts_config();
    updated.format();

    match updated.try_save().await {
        Ok(saved) => {
            info!("Wrote containerd config at {:?}", saved)
        }
        Err(err) => {
            error!("Could not save containerd config, {err}");
        }
    }
}
