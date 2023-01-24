use std::{collections::{BTreeSet, BTreeMap}, path::{Path, PathBuf}};
use logos::Logos;

/// Struct for creating a hosts.toml file for containerd hosts configuration,
/// 
pub struct HostsConfig {
    /// If set, this will be the upstream server fallback if all hosts cannot be used
    server: String, 
    /// List of hosts in priority order that will be used to serve registry requests
    hosts: Vec<Host>,
}

/// Host capabilities for configuring hosts.toml
/// 
#[derive(Logos)]
pub enum HostCapability {
    /// Resolve means the host can resolve a tag to a digest
    /// 
    #[token("resolve")]
    Resolve,
    /// Push means that the host can push content to the registry
    /// 
    #[token("push")]
    Push,
    /// Pull means that the host can pull content from a registry
    /// 
    #[token("pull")]
    Pull,
    /// Unknown token
    /// 
    #[error]
    #[regex(r"[ ,\t\n\f]+", logos::skip)]
    Error,
}

struct Host {
    /// Host URI that will be the base for registry requests,
    host: String,
    /// Supported registry features this host can serve, ex. resolve, pull, push
    features: BTreeSet<HostCapability>,
    /// If the host URI protocol is over http, this will need to be set to true in order to allow http
    skip_verify: bool, 
    /// .crt name or absolute path to .crt to support TLS (https) connections to the host
    ca: Option<Path>,
    /// Headers to pass w/ registry requests to this host
    headers: Option<BTreeMap<String, String>>,
}

impl Host {
    fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            features: BTreeSet::default(),
            skip_verify: false,
            ca: None,
            headers: None,
        }
    }

    fn skip_verify(mut self) -> Self {
        self.skip_verify = true;
        self
    }

    fn enable_pull(mut self) -> Self {
        self.enable_capability(HostCapability::Pull)
    }

    fn enable_push(mut self) -> Self {
        self.enable_capability(HostCapability::Push)
    }

    fn enable_resolve(mut self) -> Self {
        self.enable_capability(HostCapability::Resolve)
    }

    fn enable_ca(mut self, crt: impl AsRef<PathBuf>) -> Self {
        self.ca = Some(crt.as_ref().as_path().clone());
    }

    fn add_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Some(headers) = self.headers.as_mut() {
            headers.insert(key, value)
        } else {
            let mut headers = BTreeMap::default();
            headers.insert(key, value);
            self.headers = Some(headers);
        }

        self
    }

    fn enable_capability(mut self, capability: HostCapability) -> Self {
        self.features.insert(capability);
        self
    }

    fn create_config(&self) -> Result<String, std::io::Error> {
        todo!()
    }
}

impl HostsConfig {
    fn write() -> String {
        let mut hosts_config = Map::new();

        let app_hosts = properties
            .property("app_host")
            .and_then(|p| p.symbol_vec())
            .expect("app_host is required for mirror");

        for app_host in app_hosts {
            let feature_name = format!("feature_{}", app_host);
            let features = properties
                .property(feature_name)
                .and_then(|p| p.symbol_vec())
                .unwrap_or(vec![]);
            let mut host_settings = Map::new();
            let features = toml::Value::Array(
                features
                    .iter()
                    .map(|f| toml::Value::String(f.to_string()))
                    .collect::<Vec<_>>(),
            );
            host_settings.insert("capabilities".to_string(), features);
            let https = properties.property("https").and_then(|p| p.symbol());
            if let Some(https) = https {
                let host_key = format!(r#"host."https://{}""#, app_host);
                host_settings
                    .insert("ca".to_string(), toml::Value::String(https.to_string()));
                hosts_config.insert(host_key, toml::Value::Table(host_settings));
            } else {
                let host_key = format!(r#"host."http://{}""#, app_host);
                host_settings
                    .insert("skip_verify".to_string(), toml::Value::Boolean(true));
                hosts_config.insert(host_key, toml::Value::Table(host_settings));
            }
        }

        let mut content = toml::ser::to_string(&hosts_config)
            .expect("should serialize")
            .lines()
            .map(|l| {
                if l.trim().starts_with("[") {
                    l.replace(r#"[""#, "[")
                        .replace(r#"\""#, r#"""#)
                        .replace(r#""]"#, "]")
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>();

        let server = properties.property("server").and_then(|p| p.symbol());
        if let Some(server) = server {
            content.insert(0, format!(r#"server = "{server}""#));
            content.insert(1, String::default());
        }

        content.join('\n')
    }
}