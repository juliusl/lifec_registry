use crate::{HostsConfig, RegistryHost};


/// Pointer struct for creating a hosts config for a mirror host,
/// 
pub struct MirrorHost;

impl MirrorHost {
    /// Returns a host config for a mirror host,
    /// 
    pub fn get_hosts_config(server: impl Into<String>, host: impl Into<String>, insecure: bool, upgrade_streamable_format: Option<impl Into<String>>) -> HostsConfig {
        let config = HostsConfig::new(Some(server));

        let mut host = RegistryHost::new(host).enable_resolve().enable_pull();

        if insecure {
            host = host.skip_verify();
        }

        if let Some(format) = upgrade_streamable_format {
            host = host.add_header(crate::consts::UPGRADE_IF_STREAMABLE_HEADER, format.into());
        }

        config.add_host(host)
    }
}