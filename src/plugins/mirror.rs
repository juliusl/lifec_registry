use hyper::Uri;
use lifec::prelude::{
    AttributeIndex, BlockObject, BlockProperties, Component, CustomAttribute, HashMapStorage,
    Interpreter, Plugin, ThunkContext, Value, World, Block, AttributeParser,
};
use lifec_poem::AppHost;
use logos::Logos;
use std::{path::PathBuf, str::FromStr};
use toml::value::Map;
use tracing::{event, Level};
use crate::RegistryProxy;

mod host_capabilities;
use host_capabilities::HostCapability;


/// Designed to be used w/ containerd's registry config described here:
/// https://github.com/containerd/containerd/blob/main/docs/hosts.md
///
/// To enable this feature, it consists of writing a hosts.toml under /etc/containerd/certs.d/{host_name}
///
/// Here is an example to run a simple test w/ this mirror:
/// ```toml
/// server = "https://registry-1.docker.io"
///
/// [host."http://localhost:5000"]
/// capabilities = [ "resolve", "pull" ]
/// skip_verify = true
/// ```
///
/// And, then to test, you can use ctr:
/// ```sh
/// sudo ctr images pull --hosts-dir "/etc/containerd/certs.d" docker.io/library/python:latest  
/// ```
///
/// To setup the runtime, you can enable this setting in /etc/containerd/config.toml
///
/// ```toml
/// config_path = "/etc/containerd/certs.d"
/// ```
///
#[derive(Component, Clone, Default)]
#[storage(HashMapStorage)]
pub struct Mirror;

impl Mirror {
    /// Ensures the hosts dir exists for the given registry exists,
    ///
    async fn ensure_hosts_dir(app_host: impl AsRef<str>) {
        let hosts_dir = format!("/etc/containerd/certs.d/{}/", app_host.as_ref());

        let path = PathBuf::from(hosts_dir);
        if !path.exists() {
            event!(
                Level::DEBUG,
                "hosts directory did not exist, creating {:?}",
                &path
            );
            match tokio::fs::create_dir_all(&path).await {
                Ok(_) => {
                    event!(Level::DEBUG, "Created hosts directory");
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not create directories {err}");
                }
            }
        }

        let path = path.join("hosts.toml");
        if !path.exists() {
            let output_hosts_toml = PathBuf::from(format!(
                ".work/etc/containerd/certs.d/{}/hosts.toml",
                app_host.as_ref()
            ));
            event!(
                Level::DEBUG,
                "hosts.toml did not exist, creating {:?}",
                &path
            );

            assert!(
                output_hosts_toml.exists(),
                "should have been created before this plugin runs"
            );

            match tokio::fs::copy(output_hosts_toml, &path).await {
                Ok(_) => {
                    event!(Level::INFO, "Copied hosts.toml tp {:?}", path);
                }
                Err(err) => {
                    panic!("Could not copy hosts.toml, {err}");
                }
            }
        }
    }
}

impl Plugin for Mirror {
    fn symbol() -> &'static str {
        "mirror"
    }

    fn description() -> &'static str {
        "Hosts a registry mirror, to extend registry capabilities at runtime"
    }

    fn caveats() -> &'static str {
        r#"
hosts.toml must have already been installed on the machine

Design of containerd registry mirror feature
1. Add config to /etc/containerd/certs.d/{host_name}/hosts.toml
2. Content of hosts.toml
    server = "{host_name}" 

    [host."https://{address}"]
      capabilities = ["pull", "resolve"]
      ca = "path/to/{address}.crt"
"#
    }

    fn call(context: &mut ThunkContext) -> Option<lifec::plugins::AsyncContext> {
        context.task(|_| {
            let mut tc = context.clone();
            async move {
                if !tc.search().find_bool("skip_hosts_dir_check").unwrap_or_default() {
                    let app_host = tc
                        .state()
                        .find_symbol("mirror")
                        .expect("host name to mirror is required");

                    Self::ensure_hosts_dir(app_host).await;
                }

                let app_host = tc.search().find_symbol("app_host").expect("should have an app host");

                match AppHost::<RegistryProxy>::call(&mut tc.with_symbol("app_host", app_host)) {
                    Some((task, _)) => match task.await {
                        Ok(tc) => {
                            event!(Level::INFO, "Exiting");
                            Some(tc)
                        }
                        Err(err) => {
                            event!(Level::ERROR, "Error from app_host {err}");
                            None
                        }
                    },
                    _ => None,
                }
            }
        })
    }

    /// This will add some custom attributes to the parser for handling environment setup,
    ///
    /// # Usage Example
    ///
    /// ```runmd
    /// ``` test containerd
    /// + .runtime
    /// : .mirror   azurecr.io
    /// : .server   https://example.azurecr.io
    /// : .host     localhost:5000, pull, resolve, push
    /// : .https    hosts.crt
    /// ```
    ///
    fn compile(parser: &mut AttributeParser) {
        // This attribute handles setting the
        parser.add_custom(CustomAttribute::new_with(
            "server",
            |p, content| match Uri::from_str(&content) {
                Ok(upstream) => {
                    let last = p.last_child_entity().expect("child required to edit");
                    p.define_child(last, "server", Value::Symbol(upstream.to_string()));
                }
                Err(err) => {
                    event!(Level::ERROR, "Could not parse uri {}, {err}", content);
                }
            },
        ));

        parser.add_custom(CustomAttribute::new_with("host", |p, content| {
            let args = content.split_once(",");

            if let Some((proxy_to, capabilities)) = args {
                let last = p
                    .last_child_entity()
                    .expect("child entity required to edit");
                p.define_child(last, "app_host", Value::Symbol(proxy_to.to_string()));

                let mut lexer = HostCapability::lexer(capabilities);
                let feature_name = format!("feature_{}", proxy_to);
                while let Some(feature) = lexer.next() {
                    match feature {
                        HostCapability::Resolve => {
                            p.define_child(
                                last,
                                &feature_name,
                                Value::Symbol("resolve".to_string()),
                            );
                        }
                        HostCapability::Push => {
                            p.define_child(last, &feature_name, Value::Symbol("push".to_string()));
                        }
                        HostCapability::Pull => {
                            p.define_child(last, &feature_name, Value::Symbol("pull".to_string()));
                        }
                        HostCapability::Error => continue,
                    }
                }
            }
        }));

        parser.add_custom(CustomAttribute::new_with("https", |p, content| {
            let path = PathBuf::from(content);
            let path = path.canonicalize().expect("must exist");
            let last = p.last_child_entity().expect("child entity required");
            p.define_child(last, "https", Value::Symbol(format!("{:?}", path)));
        }));

        parser.with_custom::<RegistryProxy>();
    }
}

impl Interpreter for Mirror {
    fn initialize(&self, _world: &mut World) {
        // TODO
    }

    fn interpret(&self, _world: &World, block: &Block) {
        // Only interpret blocks with mirror symbol
        if block.symbol() == "mirror" && !block.name().is_empty() {
            let output_dir = PathBuf::from(".work/etc/containerd/certs.d");
            for i in block
                .index()
                .iter()
                .filter(|i| i.root().name() == "runtime")
            {
                /*
                Generate hosts.toml files for all mirrors found in state
                Example hosts.toml -
                ```toml
                server = "https://registry-1.docker.io"

                [host."http://192.168.31.250:5000"]
                capabilities = ["pull", "resolve", "push"]
                skip_verify = true
                ```
                */
                for (_, properties) in i
                    .iter_children()
                    .filter(|c| c.1.property("mirror").is_some())
                {
                    let host_name = properties
                        .property("mirror")
                        .and_then(|p| p.symbol())
                        .expect("host name is required");

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

                    let output_dir = output_dir.join(host_name);
                    std::fs::create_dir_all(&output_dir).expect("should be able to create dirs");

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

                    std::fs::write(output_dir.join("hosts.toml"), content.join("\n"))
                        .expect("should be able to write");
                }
            }
        }
    }
}

impl BlockObject for Mirror {
    fn query(&self) -> BlockProperties {
        BlockProperties::default().require("mirror")
    }

    fn parser(&self) -> Option<CustomAttribute> {
        Some(Self::as_custom_attr())
    }
}
