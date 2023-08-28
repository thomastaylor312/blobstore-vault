//! Configuration for vault blobstore capability provider
//!
use std::collections::HashMap;
use url::Url;

const DEFAULT_VAULT_ADDR: &str = "http://127.0.0.1:8200";

/// Vault configuration
#[derive(Clone, Debug)]
pub struct Config {
    /// Token for connecting to vault, can be set in environment with VAULT_TOKEN.
    /// Required
    pub token: String,
    /// Url for connecting to vault, can be set in environment with VAULT_ADDR.
    /// Defaults to 'http://127.0.0.1:8200'
    pub addr: Url,
    /// Vault mount point, can be set with in environment with VAULT_MOUNT.
    /// Defaults to "secret/"
    pub mount: String,
    /// certificate files - path to CA certificate file(s). Setting this enables TLS
    /// The linkdef value `certs` and the environment variable `VAULT_CERTS`
    /// are parsed as a comma-separated string of file paths to generate this list.
    pub certs: Vec<String>,
}

impl Default for Config {
    /// default constructor - Gets all values from environment & defaults
    fn default() -> Self {
        Self::from_values(&[]).unwrap()
    }
}

impl Config {
    /// initialize from linkdef values, environment, and defaults
    pub fn from_values(values: &[(String, String)]) -> anyhow::Result<Config> {
        let mut values: HashMap<String, String> = values.iter().cloned().collect();
        let config = Config {
            addr: values
                .remove("addr")
                .or_else(|| values.remove("ADDR"))
                .unwrap_or_else(|| DEFAULT_VAULT_ADDR.to_string())
                .parse()
                .unwrap_or_else(|_| {
                    eprintln!(
                        "Could not parse VAULT_ADDR as Url, using default of {}",
                        DEFAULT_VAULT_ADDR
                    );
                    DEFAULT_VAULT_ADDR.parse().unwrap()
                }),
            token: values
                .remove("token")
                .or_else(|| values.remove("TOKEN"))
                .ok_or_else(|| anyhow::anyhow!("missing setting for 'token' or VAULT_TOKEN"))?,
            mount: values
                .remove("mount")
                .or_else(|| values.remove("MOUNT"))
                .unwrap_or_else(|| "secret".to_string()),
            certs: match values.remove("certs").or_else(|| values.remove("CERTS")) {
                Some(certs) => certs.split(',').map(|s| s.trim().to_string()).collect(),
                _ => Vec::new(),
            },
        };
        Ok(config)
    }
}
