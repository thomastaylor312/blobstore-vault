#[derive(thiserror::Error, Debug)]
pub enum VaultError {
    /// Key not found error.
    /// Vault sometimes return 404/not found error for other causes such as requester not having
    /// authorization, and NotFound is used to avoid leaking too much info to an attacker.
    #[error("Key not found: namespace/key {namespace}/{path}")]
    NotFound { namespace: String, path: String },

    /// All other errors
    #[error("An error occurred with the request")]
    Client(#[from] vaultrs::error::ClientError),
}