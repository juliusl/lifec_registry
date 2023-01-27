use crate::{RegistryHost, HostsConfig};


/// Pointer struct for host config implementation for a default host mirror,
/// 
/// A default mirror is responsible for detecting whether or not an incoming registry request can be accepted
/// 
pub struct DefaultHost;

impl DefaultHost {
    /// Returns the hosts config for the default host mirror,
    /// 
    pub fn get_hosts_config(address: impl Into<String>, insecure: bool, suffix_match: Option<impl Into<String>>, streamable_format: Option<impl Into<String>>) -> HostsConfig {
        let config = HostsConfig::new(None::<String>);

        let mut host = RegistryHost::new(address.into())
            .enable_resolve();
        
        if insecure {
            host = host.skip_verify();
        }

        if let Some(suffix_match) = suffix_match {
            let suffix_match = suffix_match.into();
            host = host.add_header(crate::consts::ACCEPT_IF_SUFFIX_HEADER, suffix_match.clone());
            host = host.add_header(crate::consts::ENABLE_MIRROR_IF_SUFFIX_HEADER, suffix_match);
        }

        if let Some(streamable_format) = streamable_format {
            host = host.add_header(crate::consts::UPGRADE_IF_STREAMABLE_HEADER, streamable_format.into());
        }

        config.add_host(host)
    }
}