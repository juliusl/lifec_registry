use logos::Logos;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    path::PathBuf, io::Write
};
use tracing::error;

/// Folder name of the default hosts_config folder used by containerd,
/// 
const HOSTS_CONFIG_FOLDER: &'static str = "etc/containerd/certs.d";

/// Struct for creating a hosts.toml file for containerd hosts configuration,
///
pub struct HostsConfig {
    /// If set, this will be the upstream server fallback if all hosts cannot be used,
    /// https is hardcoded protocol is hardcoded for this
    server: Option<String>,
    /// List of hosts in priority order that will be used to serve registry requests
    hosts: Vec<Host>,
    /// If true, adds logic to handle configuring a hosts.toml for containerd versions under 1.7
    legacy_support: bool,
}

impl HostsConfig {
    /// Returns a new hosts config that can serialize into compatible toml for containerd hosts.toml feature,
    ///
    pub fn new(server: Option<impl Into<String>>) -> Self {
        Self {
            server: server.map(|s| s.into()),
            hosts: vec![],
            legacy_support: false,
        }
    }

    /// Enables legacy support for containerd version under 1.7
    /// 
    pub fn enable_legacy_support(mut self) -> Self {
        self.legacy_support = true;
        self
    }

    /// Adds host to list of hosts, chainable
    ///
    pub fn add_host(mut self, host: Host) -> Self {
        self.hosts.push(host);
        self
    }

    /// Serializes and writes the current config to
    /// 
    pub fn install(&self, root_dir: Option<impl Into<PathBuf>>) -> Result<PathBuf, std::io::Error> {
        let path = root_dir.map(|r| r.into()).unwrap_or(PathBuf::from("/"));
        let path = path.join(HOSTS_CONFIG_FOLDER);

        let path = if let Some(server) = self.server.as_ref() {
            path.join(server)
        } else {
            path.join("_default")
        };

        std::fs::create_dir_all(&path)?;

        let path = path.join("hosts.toml");

        let mut file = std::fs::File::create(&path)?;

        file.write_all(format!("{}", self).as_bytes())?;

        // TODO -- Make readonly?

        Ok(path)
    }
}

impl Display for HostsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Workaround for _default host being only available in ctrd 1.7 +, a server should never start with azurecr
        if let Some(server) = self.server.as_ref().filter(|_| !self.legacy_support) {
            writeln!(f, r#"server = "https://{}""#, server)?;
            writeln!(f, "")?;
        }

        for host in self.hosts.iter() {
            writeln!(f, "{}", host)?;
            writeln!(f, "")?;
        }

        Ok(())
    }
}

/// Host capabilities for configuring hosts.toml
///
#[derive(Logos, PartialEq, PartialOrd, Ord, Eq)]
enum HostCapability {
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

impl Display for HostCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostCapability::Resolve => write!(f, "resolve"),
            HostCapability::Push => write!(f, "push"),
            HostCapability::Pull => write!(f, "pull"),
            HostCapability::Error => panic!("value must be either 'resolve', 'push', or 'pull'"),
        }
    }
}

/// Struct that defines properties of a Hosts config file,
///
pub struct Host {
    /// Host URI that will be the base for registry requests,
    host: String,
    /// Supported registry features this host can serve, ex. resolve, pull, push
    features: BTreeSet<HostCapability>,
    /// If the host URI protocol is over http, this will need to be set to true in order to allow http
    skip_verify: bool,
    /// .pem/.crt name or absolute path to .pem/.crt to support TLS (https) connections to the host
    ca: Option<PathBuf>,
    /// .pem/.crt name or absolute path to .pem/.crt to support client cert authentication w/ the host
    client: Option<(PathBuf, Option<PathBuf>)>,
    /// Headers to pass w/ registry requests to this host
    headers: Option<BTreeMap<String, String>>,
}

impl Host {
    /// Returns a new host config,
    ///
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            features: BTreeSet::default(),
            skip_verify: false,
            ca: None,
            client: None,
            headers: None,
        }
    }

    /// Enables the skip_verify option to support http connections, chainable
    ///
    pub fn skip_verify(mut self) -> Self {
        self.skip_verify = true;
        self
    }

    /// Enables pull capability, chainable
    ///
    pub fn enable_pull(self) -> Self {
        self.enable_capability(HostCapability::Pull)
    }

    /// Enables push capability, chainable
    ///
    pub fn enable_push(self) -> Self {
        self.enable_capability(HostCapability::Push)
    }

    /// Enables resolve capability, chainable
    ///
    pub fn enable_resolve(self) -> Self {
        self.enable_capability(HostCapability::Resolve)
    }

    /// Enables the ca option to enable TLS support, chainable
    ///
    pub fn enable_ca(mut self, ca: impl Into<PathBuf>) -> Self {
        self.ca = Some(ca.into());
        self
    }

    /// Enables the client option, to enable client cert authn, chainable
    ///
    pub fn enable_client(
        mut self,
        client: impl Into<PathBuf>,
        key: Option<impl Into<PathBuf>>,
    ) -> Self {
        self.client = Some((client.into(), key.map(|k| k.into())));
        self
    }

    /// Enables header option to pass w/ each registry request,
    ///
    pub fn add_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Some(headers) = self.headers.as_mut() {
            headers.insert(key.into(), value.into());
        } else {
            let mut headers = BTreeMap::default();
            headers.insert(key.into(), value.into());
            self.headers = Some(headers);
        }

        self
    }

    fn enable_capability(mut self, capability: HostCapability) -> Self {
        self.features.insert(capability);
        self
    }
}

impl Display for Host {
    /// We can't directly serialize the struct type because the hosts have a specific order that they must be declared in
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The header for a host is in format of `[host."<server-uri>"]`
        let host_header = format!(r#"[host."{}"]"#, self.host);
        writeln!(f, "{}", host_header)?;

        let mut host_capabilities = toml::value::Array::new();

        for c in self.features.iter() {
            host_capabilities.push(toml::Value::String(c.to_string()));
        }

        let host_capabilities = toml::to_string(&host_capabilities);
        match host_capabilities {
            Ok(c) => {
                writeln!(f, r#"  capabilities = {}"#, c)?;
            }
            Err(err) => {
                error!("Error serializing host capabilities, {err}");
                return Err(std::fmt::Error {});
            }
        }

        if self.skip_verify {
            writeln!(f, r#"  skip_verify = true"#)?;
        } else if let Some(ca) = self.ca.as_ref() {
            writeln!(f, r#"  ca = {:?}"#, ca)?;
        } else if self.host.starts_with("http://") {
            error!(
                "Host {} is listening w/ http but did not enable skip_verify",
                self.host
            );
        }

        if let Some((client, client_key)) = self.client.as_ref() {
            writeln!(
                f,
                r#"  client = [{:?}, {:?}]"#,
                client,
                client_key.as_ref().unwrap_or(&PathBuf::default())
            )?;
        }

        if let Some(headers) = self.headers.as_ref() {
            writeln!(f, r#"  [host."{}".header]"#, self.host)?;

            for (name, value) in headers.iter() {
                writeln!(f, r#"    {} = "{}""#, name, value)?;
            }
        }

        Ok(())
    }
}

mod tests {
    #[test]
    fn test_display_host_config() {
        use crate::HostsConfig;

        // Test w/ server=
        let host_config = HostsConfig::new(Some("test.azurecr.io"));
        let host_config = host_config.add_host(
            super::Host::new("http://localhost:6879")
                .enable_resolve()
                .enable_pull()
                .skip_verify()
                .add_header("x-ms-acr-tenant", "test")
                .add_header("x-ms-acr-tenant-host", "azurecr.io"),
        );

        assert_eq!(
            r#"
server = "https://test.azurecr.io"

[host."http://localhost:6879"]
  capabilities = ["resolve", "pull"]
  skip_verify = true
  [host."http://localhost:6879".header]
    x-ms-acr-tenant = "test"
    x-ms-acr-tenant-host = "azurecr.io"


"#
            .trim_start(),
            format!("{}", host_config)
        );
        let location = host_config.install(Some(".test")).expect("should be able to install");
        eprintln!("{:?}", location);
        
        // Test w/o server=
        let host_config = HostsConfig::new(None::<String>);
        let host_config = host_config.add_host(
            super::Host::new("http://localhost:6879")
                .enable_resolve()
                .enable_pull()
                .skip_verify()
                .add_header("x-ms-acr-tenant", "test")
                .add_header("x-ms-acr-tenant-host", "azurecr.io"),
        );
        assert_eq!(
            r#"
[host."http://localhost:6879"]
  capabilities = ["resolve", "pull"]
  skip_verify = true
  [host."http://localhost:6879".header]
    x-ms-acr-tenant = "test"
    x-ms-acr-tenant-host = "azurecr.io"


"#
            .trim_start(),
            format!("{}", host_config)
        );

        let location = host_config.install(Some(".test")).expect("should be able to install");
        eprintln!("{:?}", location);
    }

    #[test]
    #[tracing_test::traced_test]
    fn test_display_host() {
        use super::Host;
        use std::path::PathBuf;

        let host = Host::new("http://localhost:6879");

        let host = host
            .enable_resolve()
            .enable_pull()
            .skip_verify()
            .add_header("x-ms-acr-tenant", "test")
            .add_header("x-ms-acr-tenant-host", "azurecr.io");

        let toml_config = format!("{}", host);

        assert_eq!(
            r#"
[host."http://localhost:6879"]
  capabilities = ["resolve", "pull"]
  skip_verify = true
  [host."http://localhost:6879".header]
    x-ms-acr-tenant = "test"
    x-ms-acr-tenant-host = "azurecr.io"
"#
            .trim_start(),
            toml_config
        );

        // Test logging error when skip_verify isn't included
        let host = Host::new("http://localhost:6879");
        let host = host
            .enable_resolve()
            .enable_pull()
            .add_header("x-ms-acr-tenant", "test")
            .add_header("x-ms-acr-tenant-host", "azurecr.io");
        let _ = format!("{}", host);
        assert!(logs_contain(
            "is listening w/ http but did not enable skip_verify"
        ));

        let host = Host::new("https://localhost:6879");

        let host = host
            .enable_resolve()
            .enable_pull()
            .enable_ca("test.pem")
            .enable_client("test-client.pem", None::<PathBuf>)
            .add_header("x-ms-acr-tenant", "test")
            .add_header("x-ms-acr-tenant-host", "azurecr.io");

        let toml_config = format!("{}", host);

        assert_eq!(
            r#"
[host."https://localhost:6879"]
  capabilities = ["resolve", "pull"]
  ca = "test.pem"
  client = ["test-client.pem", ""]
  [host."https://localhost:6879".header]
    x-ms-acr-tenant = "test"
    x-ms-acr-tenant-host = "azurecr.io"
"#
            .trim_start(),
            toml_config
        );
    }
}
