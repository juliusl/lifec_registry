use std::path::PathBuf;

use toml_edit::{Document, Table};

use crate::Error;

/// Struct that reads a more traditional docker login config,
/// 
/// **Note** Should be placed in `/etc/acr-mirror/login.toml`
/// 
/// Example:
/// 
/// ```toml
/// [auth."<host>"]
/// username = <username>
/// password = <password>
/// 
/// ```
#[derive(Default)]
pub struct LoginConfig {
    /// Auth table
    /// 
    doc: toml_edit::Document,
    /// Root config dir,
    /// 
    root: PathBuf,
}

/// Default directory to use for config,
/// 
const DEFAULT_ROOT_CONFIG_PATH: &'static str = "/etc/acr-mirror/";

/// Config file name,
/// 
const CONFIG_NAME: &'static str = "login.toml";

impl LoginConfig {
    /// Creates a new login config, or loads an existing one
    /// 
    pub fn load(root: Option<PathBuf>) -> Result<Self, Error> {
        let root = root.unwrap_or(PathBuf::from(DEFAULT_ROOT_CONFIG_PATH));
        let mut config = Self { doc: toml_edit::Document::new(), root };
        std::fs::create_dir_all(&config.root)?;

        let path = config.root.join(CONFIG_NAME);

        if path.exists() {
            if let Ok(doc) = std::fs::read_to_string(path)?.parse::<Document>() {
                config.doc = doc;
            }
        }

        if !config.doc.get_mut("auth").map(|t| t.is_table()).unwrap_or_default() {
            config.doc["auth"] = toml_edit::table();
        }

        config.doc["auth"].as_table_mut().map(|t| t.set_implicit(true));

        Ok(config)
    }

    /// Adds a new login to config and writes to file,
    /// 
    pub fn login(&mut self, host: impl AsRef<str>, username: impl Into<String>, password: impl Into<String>) -> Result<bool, Error> {
        let mut login = Table::new();
        login.set_implicit(true);
        login["username"] = toml_edit::value(username.into());
        login["password"] = toml_edit::value(password.into());

        let existed = self.doc["auth"].as_table().map(|t| t.contains_table(host.as_ref())).unwrap_or_default();
        // This will clear any existing login for this host
        self.doc["auth"].as_table_mut().map(|t| t.insert(host.as_ref(), toml_edit::Item::Table(login)));

        self.save_to_disk()?;

        Ok(existed)
    }

    /// Authorizes a host,
    /// 
    pub fn authorize(&self, host: impl AsRef<str>) -> Option<(&str, &str)> {
        self.doc["auth"].as_table().and_then(|t| t.get(host.as_ref()).and_then(|v| v.as_table()).and_then(|t| {
            if let (Some(u), Some(p)) = (t["username"].as_str(), t["password"].as_str()) {
                Some((u, p))
            } else {
                None
            }
        }))
    }

    /// Saves login to disk,
    /// 
    pub fn save_to_disk(&self) -> Result<(), Error> {
        let path = self.root.join(CONFIG_NAME);

        std::fs::write(&path, format!("{}", self.doc))?;
        Ok(())    
    }
}

#[allow(unused_imports)]
mod tests {
    use super::LoginConfig;

    #[test]
    fn test_login_config() {
        let mut config = LoginConfig::load(Some(".test_login".into())).unwrap();

        let overwritten = config.login("test.endpoint.io", "username", "password").unwrap();
        assert!(!overwritten);

        let (u, p) = config.authorize("test.endpoint.io").unwrap();
        assert_eq!("username", u);
        assert_eq!("password", p);

        std::fs::remove_dir_all(".test_login").unwrap();
    }
}