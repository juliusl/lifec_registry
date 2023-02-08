use std::{fmt::Display, path::PathBuf};

use toml_edit::{table, value, Document};

use crate::Error;

const DEFAULT_CONTAINERD_CONFIG_DIR: &'static str = "etc/containerd/";

/// Pointer struct for handling /etc/containerd/config.toml
///
pub struct ContainerdConfig {
    /// Internal toml document for editing config,
    ///
    document: toml_edit::Document,
    /// Sets the root directory, for test purposes
    ///
    _root_dir: Option<&'static str>,
}

impl ContainerdConfig {
    /// Returns an empty default containerd config,
    ///
    pub fn new() -> Self {
        Self {
            document: Document::new(),
            _root_dir: None,
        }
        .ensure_common_tables()
    }

    /// Tries to save the current content to the default containerd config path,
    ///
    pub async fn try_save(&self) -> Result<PathBuf, Error> {
        let path = PathBuf::from(self._root_dir.unwrap_or("/"));
        let path = path.join(DEFAULT_CONTAINERD_CONFIG_DIR);

        tokio::fs::create_dir_all(&path).await?;
        let path = path.join("config.toml");

        tokio::fs::write(&path, format!("{}", self)).await?;
        Ok(path)
    }

    /// Tries to load an existing config from the filesystem,
    ///
    /// Returns an error if the file does not exist, the file cannot be read, or the content is invalid
    ///
    pub async fn try_load(root: Option<&'static str>) -> Result<Self, Error> {
        let path = PathBuf::from(root.unwrap_or("/"))
            .join(DEFAULT_CONTAINERD_CONFIG_DIR)
            .join("config.toml");
        let path = path.canonicalize()?;
        let content = tokio::fs::read_to_string(path).await?;
        let document = content.parse::<Document>()?;

        Ok(Self {
            document,
            _root_dir: None,
        }
        .ensure_common_tables())
    }

    /// Tries to load content as a toml document, if successful overrides the current document,
    ///
    pub fn try_load_content(mut self, content: impl AsRef<str>) -> Result<Self, Error> {
        let document = content.as_ref().parse::<Document>()?;

        self.document = document;
        Ok(self.ensure_common_tables())
    }

    /// Enables the hosts config feature,
    ///
    pub fn enable_hosts_config(mut self) -> Self {
        self.document["plugins"]["io.containerd.grpc.v1.cri"]["registry"]["config_path"] =
            value("/etc/containerd/certs.d");
        self
    }

    /// Enables proxy plugin overlaybd snapshotter and configures it as the default snapshotter,
    ///
    pub fn enable_overlaybd_snapshotter(mut self) -> Self {
        self.document["proxy_plugins"]["overlaybd"]["type"] = value("snapshot");
        self.document["proxy_plugins"]["overlaybd"]["address"] =
            value("/run/overlaybd-snapshotter/overlaybd.sock");

        self.enable_default_cri_snapshotter("overlaybd", true)
    }

    /// Enables a default snapshotter for cri,
    ///
    /// If remote is true, sets `disable_snapshot_annotations` to false
    ///
    fn enable_default_cri_snapshotter(
        mut self,
        snapshotter: impl AsRef<str>,
        remote: bool,
    ) -> Self {
        // [plugins."io.containerd.grpc.v1.cri".containerd]
        // snapshotter = "overlaybd"
        // disable_snapshot_annotations = false

        self.document["plugins"]["io.containerd.grpc.v1.cri"]["containerd"]["snapshotter"] =
            value(snapshotter.as_ref());

        if remote {
            self.document["plugins"]["io.containerd.grpc.v1.cri"]["containerd"]
                ["disable_snapshot_annotations"] = value(false);
        }
        self
    }

    /// Ensures common tables are initialized,
    ///
    fn ensure_common_tables(mut self) -> Self {
        self.document["plugins"].enable_default_table();
        self.document["plugins"]["io.containerd.grpc.v1.cri"].enable_default_table();
        self.document["plugins"]["io.containerd.grpc.v1.cri"]["containerd"].enable_default_table();
        self.document["plugins"]["io.containerd.grpc.v1.cri"]["registry"].enable_default_table();
        self.document["proxy_plugins"].enable_default_table();
        self.document["proxy_plugins"]["overlaybd"].enable_default_table();
        self
    }

    /// Formats the document,
    /// 
    pub fn format(&mut self) {
        self.document["plugins"].default_format();
        self.document["plugins"]["io.containerd.grpc.v1.cri"].default_format();
        self.document["proxy_plugins"].default_format();
        self.document["proxy_plugins"]["overlaybd"].default_format();
    }

    /// Sets the root directory for th
    ///
    #[allow(dead_code)]
    fn root_dir(mut self, root: &'static str) -> Self {
        self._root_dir = Some(root);
        self
    }
}

impl Display for ContainerdConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.document)
    }
}

trait EnableDefault {
    fn enable_default_table(&mut self);

    fn default_format(&mut self);

    fn default_child_format(&mut self);
}

impl EnableDefault for toml_edit::Item {
    fn enable_default_table(&mut self) {
        if self.as_table().is_none() {
            *self = table();
            self.as_table_mut().map(|t| t.set_implicit(true));
        }
    }

    fn default_format(&mut self) {
        self.as_table_mut().map(|t| { 
            for (mut key, item) in t.iter_mut() {
                item.as_value_mut().map(|_| key.decor_mut().set_prefix("    "));

                if item.as_table_mut().is_some() {
                    item.default_child_format();
                }
            }
        });
    }

    fn default_child_format(&mut self) {
        self.as_table_mut().map(|t| { 
            for (mut key, item) in t.iter_mut() {
                item.as_value_mut().map(|_| key.decor_mut().set_prefix("        "));
            }
        });
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::{config::containerd_config::EnableDefault, ContainerdConfig};

    /// Tests that an existing config can be edited,
    ///
    /// The example used in this test is a typical default AKS containerd config,
    ///
    #[test]
    fn test_edit_containerd_config() {
        let config_content = r#"version = 2
oom_score = 0
            
[plugins."io.containerd.grpc.v1.cri"]
    sandbox_image = "mcr.microsoft.com/oss/kubernetes/pause:3.6"
    [plugins."io.containerd.grpc.v1.cri".containerd]
        default_runtime_name = "runc"
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc]
        runtime_type = "io.containerd.runc.v2"
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc.options]
        BinaryName = "/usr/bin/runc"
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.untrusted]
        runtime_type = "io.containerd.runc.v2"
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.untrusted.options]
        BinaryName = "/usr/bin/runc"
    [plugins."io.containerd.grpc.v1.cri".cni]
        bin_dir = "/opt/cni/bin"
        conf_dir = "/etc/cni/net.d"
        conf_template = "/etc/containerd/kubenet_template.conf"
            
[metrics]
    address = "0.0.0.0:10257"
"#;
        let mut config = ContainerdConfig::new()
            .try_load_content(config_content)
            .expect("should be able to load content")
            .enable_hosts_config()
            .enable_overlaybd_snapshotter();

        config.format();

        assert_eq!(r#"version = 2
oom_score = 0
            
[plugins."io.containerd.grpc.v1.cri"]
    sandbox_image = "mcr.microsoft.com/oss/kubernetes/pause:3.6"
    [plugins."io.containerd.grpc.v1.cri".containerd]
        default_runtime_name = "runc"
        snapshotter = "overlaybd"
        disable_snapshot_annotations = false
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc]
        runtime_type = "io.containerd.runc.v2"
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc.options]
        BinaryName = "/usr/bin/runc"
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.untrusted]
        runtime_type = "io.containerd.runc.v2"
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.untrusted.options]
        BinaryName = "/usr/bin/runc"
    [plugins."io.containerd.grpc.v1.cri".cni]
        bin_dir = "/opt/cni/bin"
        conf_dir = "/etc/cni/net.d"
        conf_template = "/etc/containerd/kubenet_template.conf"

[plugins."io.containerd.grpc.v1.cri".registry]
        config_path = "/etc/containerd/certs.d"
            
[metrics]
    address = "0.0.0.0:10257"

[proxy_plugins.overlaybd]
    type = "snapshot"
    address = "/run/overlaybd-snapshotter/overlaybd.sock"
"#, format!("{}", config));
    }

    /// Tests that the file can be saved to disk,
    ///
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_save_containerd_config() {
        let config = ContainerdConfig::new()
            .enable_hosts_config()
            .enable_overlaybd_snapshotter()
            .root_dir(".test");

        config.try_save().await.expect("should be able to save");

        let saved = format!(
            "{}",
            tokio::fs::read_to_string(".test/etc/containerd/config.toml")
                .await
                .expect("should be able to read")
        );

        assert_eq!(
            r#"[plugins."io.containerd.grpc.v1.cri".containerd]
snapshotter = "overlaybd"
disable_snapshot_annotations = false

[plugins."io.containerd.grpc.v1.cri".registry]
config_path = "/etc/containerd/certs.d"

[proxy_plugins.overlaybd]
type = "snapshot"
address = "/run/overlaybd-snapshotter/overlaybd.sock"
"#,
            saved
        );
    }
}
