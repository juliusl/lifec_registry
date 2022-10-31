use logos::Logos;

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